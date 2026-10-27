//! Quota and rate-limit detection (ENG-02, D-15). One pure detector per stream family, called once
//! after a run ends. A hit only STOPS the step: nothing here retries, switches engine or starts
//! another run (D-16).
//!
//! The exact `result` of a real Claude Code usage-limit stop, and how DeepSeek 402/429 render
//! through Claude Code, were never captured (research A1-A3), so detection is multi-signal and the
//! raw stream lines stay in `events`; the first real stop recalibrates the rules.

use crate::stream::Summary;
use regex::Regex;
use std::sync::OnceLock;

/// `runs.outcome` of a run that stopped on quota.
pub const OUTCOME_QUOTA: &str = "quota";

const DETAIL_MAX_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct QuotaHit {
    /// Which rule fired, e.g. `rate_limit_event.rejected`; stored in `metrics_json.quota.signal`.
    pub signal: &'static str,
    pub detail: String,
    /// Reset time in Unix milliseconds, when the stream carried one.
    pub reset_at_ms: Option<i64>,
}

fn text_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)hit your (session |weekly |opus |sonnet |monthly spend )?limit|usage limit|credit balance is too low|spend limit|insufficient balance|out of balance|rate limit|too many requests|\b429\b|\b402\b",
        )
        .expect("quota text regex")
    })
}

/// Case-insensitive match of the user-visible quota / limit / balance phrases.
pub fn text_matches(s: &str) -> bool {
    text_regex().is_match(s)
}

/// Whitespace collapsed to single spaces, cut to 200 characters.
fn one_line(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(DETAIL_MAX_CHARS).collect()
}

/// First line of `s` that matches the quota phrases, in one line.
fn first_matching_line(s: &str) -> String {
    s.lines()
        .find(|l| text_matches(l))
        .map(one_line)
        .unwrap_or_else(|| one_line(s))
}

/// Quota decision for a claude-format run (claude and deepseek engines).
///
/// Rules, in order: a final result that succeeded is never a stop (an overage may have covered a
/// rejected event, and a retried 429 ends in success); a rejected `rate_limit_event` without
/// overage; an error result with HTTP 429 or 402; `terminal_reason: blocking_limit`; an error
/// result whose text names a limit or balance; without any result, a last `api_retry` of
/// `rate_limit` or `billing_error`; without any result and a non-zero exit, a limit phrase in the
/// stderr tail. A 529 `Overloaded` error matches none of these.
pub fn detect_claude(s: &Summary, exit_code: Option<i32>, stderr_tail: &str) -> Option<QuotaHit> {
    if s.result_seen() && !s.is_error {
        return None;
    }

    if s.rate_limit_rejected && !s.using_overage {
        let window = s.rate_limit_type.as_deref().unwrap_or("?");
        return Some(QuotaHit {
            signal: "rate_limit_event.rejected",
            detail: format!("rateLimitType {window} bị từ chối"),
            reset_at_ms: s.rate_limit_reset_at_ms,
        });
    }

    if s.is_error {
        if let Some(code @ (429 | 402)) = s.api_error_status {
            return Some(QuotaHit {
                signal: "result.api_error_status",
                detail: format!("HTTP {code}"),
                reset_at_ms: None,
            });
        }
    }

    if s.terminal_reason.as_deref() == Some("blocking_limit") {
        let detail = match s.result_text.as_deref().map(one_line) {
            Some(t) if !t.is_empty() => t,
            _ => "terminal_reason blocking_limit".to_string(),
        };
        return Some(QuotaHit {
            signal: "result.terminal_reason",
            detail,
            reset_at_ms: None,
        });
    }

    if s.is_error {
        if let Some(text) = s.result_text.as_deref() {
            if text_matches(text) {
                return Some(QuotaHit {
                    signal: "result.text",
                    detail: one_line(text),
                    reset_at_ms: None,
                });
            }
        }
    }

    if !s.result_seen() {
        if let Some(kind @ ("rate_limit" | "billing_error")) = s.last_retry_error.as_deref() {
            let detail = match s.last_retry_status {
                Some(code) => format!("api_retry {kind} (HTTP {code})"),
                None => format!("api_retry {kind}"),
            };
            return Some(QuotaHit {
                signal: "api_retry",
                detail,
                reset_at_ms: None,
            });
        }

        if exit_code.unwrap_or(0) != 0 && text_matches(stderr_tail) {
            return Some(QuotaHit {
                signal: "stderr",
                detail: first_matching_line(stderr_tail),
                reset_at_ms: None,
            });
        }
    }

    None
}

/// The last `max_bytes` of a file as lossy UTF-8; a missing or unreadable file gives "".
pub fn stderr_tail(path: &std::path::Path, max_bytes: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(len) = f.metadata().map(|m| m.len()) else {
        return String::new();
    };
    if len > max_bytes && f.seek(SeekFrom::Start(len - max_bytes)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if f.take(max_bytes).read_to_end(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Autopilot stop line for a quota run (`StopInfo.detail_line`).
pub fn stop_detail(run_id: i64, role: &str, engine: &str) -> String {
    format!(
        "Run {run_id} ({role}) trên engine {engine} dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine."
    )
}

/// What the role prints on stderr when it stops on quota.
pub fn user_message(engine: &str, hit: &QuotaHit) -> String {
    format!(
        "Hết quota engine {engine}: {} — agent-brain không tự đổi engine. Chọn đội hình khác (Chạy lại bước này bằng đội hình…) hoặc chạy lại sau.",
        hit.detail
    )
}

/// Vietnamese name of a step from its stage (or, without a stage, its role).
pub fn step_name(stage: Option<&str>, role: &str) -> &'static str {
    match stage {
        Some("plan") => "Lên plan",
        Some("plan-review") => "Review plan",
        Some("implement") => "Code",
        Some("code-review") => "Review code",
        Some("qa") => "QA",
        Some("fix") => "Sửa",
        _ => match role {
            "lead" => "Lên plan",
            "plan-review" => "Review plan",
            "dev" => "Code",
            "code-review" => "Review code",
            "qa" => "QA",
            _ => "Bước",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary_of(lines: &[&str]) -> Summary {
        let mut s = Summary::new(None, None);
        for l in lines {
            s.feed(l.as_bytes());
        }
        s.finish();
        s
    }

    const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"q","model":"fake"}"#;
    const OK_RESULT: &str =
        r#"{"type":"result","subtype":"success","is_error":false,"result":"xong","num_turns":1}"#;
    const REJECTED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792310400,"rateLimitType":"five_hour","isUsingOverage":false,"overageStatus":"rejected"}}"#;
    const RETRY_429: &str = r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":5,"retry_delay_ms":10,"error_status":429,"error":"rate_limit"}"#;

    fn err_result(extra: &str, text: &str) -> String {
        format!(
            r#"{{"type":"result","subtype":"success","is_error":true{extra},"result":"{text}","num_turns":1}}"#
        )
    }

    #[test]
    fn success_result_after_rejected_event_is_not_quota() {
        let s = summary_of(&[INIT, REJECTED, OK_RESULT]);
        assert_eq!(detect_claude(&s, Some(0), ""), None);
        // Even with a non-zero exit and a limit phrase on stderr, the success wins.
        assert_eq!(detect_claude(&s, Some(1), "429 Too Many Requests"), None);
    }

    #[test]
    fn allowed_warning_never_stops() {
        let warn = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1792310400,"rateLimitType":"seven_day","isUsingOverage":false}}"#;
        let err = err_result("", "boom");
        let s = summary_of(&[INIT, warn, &err]);
        assert_eq!(detect_claude(&s, Some(1), ""), None);
    }

    #[test]
    fn rejected_without_overage_stops_with_reset_time() {
        let err = err_result("", "boom");
        let s = summary_of(&[INIT, REJECTED, &err]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "rate_limit_event.rejected");
        assert_eq!(hit.reset_at_ms, Some(1_792_310_400_000));
        assert!(hit.detail.contains("five_hour"), "detail: {}", hit.detail);
    }

    #[test]
    fn rejected_without_overage_and_no_result_still_stops() {
        let s = summary_of(&[INIT, REJECTED]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "rate_limit_event.rejected");
    }

    #[test]
    fn rejected_with_overage_alone_is_not_quota() {
        let rejected_overage = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792310400,"rateLimitType":"five_hour","isUsingOverage":true}}"#;
        let err = err_result("", "boom");
        let s = summary_of(&[INIT, rejected_overage, &err]);
        assert_eq!(detect_claude(&s, Some(1), ""), None);
    }

    #[test]
    fn api_error_status_402_and_429_stop() {
        for code in [402, 429] {
            let err = err_result(&format!(r#","api_error_status":{code}"#), "boom");
            let s = summary_of(&[INIT, &err]);
            let hit = detect_claude(&s, Some(1), "").expect("hit");
            assert_eq!(hit.signal, "result.api_error_status");
            assert_eq!(hit.detail, format!("HTTP {code}"));
            assert_eq!(hit.reset_at_ms, None);
        }
    }

    #[test]
    fn api_error_status_ignored_on_a_non_error_result() {
        let ok = r#"{"type":"result","subtype":"success","is_error":false,"api_error_status":429}"#;
        let s = summary_of(&[INIT, ok]);
        assert_eq!(detect_claude(&s, Some(0), ""), None);
    }

    #[test]
    fn blocking_limit_terminal_reason_stops() {
        let err = err_result(r#","terminal_reason":"blocking_limit""#, "boom");
        let s = summary_of(&[INIT, &err]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "result.terminal_reason");
        assert_eq!(hit.detail, "boom");

        // Without any text the detail still names the reason.
        let bare = r#"{"type":"result","subtype":"success","is_error":true,"terminal_reason":"blocking_limit"}"#;
        let s = summary_of(&[INIT, bare]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "result.terminal_reason");
        assert_eq!(hit.detail, "terminal_reason blocking_limit");
    }

    #[test]
    fn error_text_naming_a_limit_stops() {
        let err = err_result("", "Credit balance is too low");
        let s = summary_of(&[INIT, &err]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "result.text");
        assert_eq!(hit.detail, "Credit balance is too low");
    }

    #[test]
    fn long_text_detail_is_cut_to_one_line_of_200_chars() {
        let long = format!("You've hit your session limit {}", "x\\n".repeat(400));
        let err = err_result("", &long);
        let s = summary_of(&[INIT, &err]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "result.text");
        assert!(hit.detail.chars().count() <= 200, "{}", hit.detail.len());
        assert!(!hit.detail.contains('\n'));
    }

    #[test]
    fn retry_rate_limit_without_result_stops() {
        let s = summary_of(&[INIT, RETRY_429]);
        let hit = detect_claude(&s, Some(1), "").expect("hit");
        assert_eq!(hit.signal, "api_retry");

        let billing =
            r#"{"type":"system","subtype":"api_retry","error_status":402,"error":"billing_error"}"#;
        let s = summary_of(&[INIT, billing]);
        assert_eq!(
            detect_claude(&s, Some(1), "").expect("hit").signal,
            "api_retry"
        );
    }

    #[test]
    fn retries_then_success_is_not_quota() {
        let s = summary_of(&[INIT, RETRY_429, RETRY_429, OK_RESULT]);
        assert_eq!(detect_claude(&s, Some(0), ""), None);
    }

    #[test]
    fn stderr_signal_needs_no_result_and_a_failed_exit() {
        let s = summary_of(&[INIT]);
        let hit = detect_claude(&s, Some(1), "warming up\nError: 429 Too Many Requests\nbye")
            .expect("hit");
        assert_eq!(hit.signal, "stderr");
        assert_eq!(hit.detail, "Error: 429 Too Many Requests");

        assert_eq!(
            detect_claude(&s, Some(1), "Error: connection refused"),
            None
        );
        // A zero exit with a limit phrase on stderr is not a stop.
        assert_eq!(detect_claude(&s, Some(0), "429 Too Many Requests"), None);
        assert_eq!(detect_claude(&s, None, "429 Too Many Requests"), None);
    }

    #[test]
    fn overloaded_529_is_not_quota() {
        let err = err_result(
            r#","api_error_status":529"#,
            "API Error: Repeated 529 Overloaded errors",
        );
        let s = summary_of(&[INIT, &err]);
        assert_eq!(detect_claude(&s, Some(1), ""), None);

        let overloaded =
            r#"{"type":"system","subtype":"api_retry","error_status":529,"error":"overloaded"}"#;
        let s = summary_of(&[INIT, overloaded]);
        assert_eq!(detect_claude(&s, Some(1), ""), None);
    }

    #[test]
    fn text_matches_phrases() {
        for yes in [
            "You've hit your session limit",
            "You've hit your weekly limit",
            "You've hit your Opus limit",
            "You've hit your monthly spend limit",
            "spend limit reached",
            "Request rejected (429)",
            "Credit balance is too low",
            "usage limit reached",
            "You have run out of balance",
            "insufficient balance",
            "Server is temporarily limiting requests: rate limit",
            "Too Many Requests",
            "HTTP 402 Payment Required",
        ] {
            assert!(text_matches(yes), "{yes}");
        }
        for no in [
            "API Error: Repeated 529 Overloaded errors",
            "connection refused",
            "error 4290 in parser",
            "",
        ] {
            assert!(!text_matches(no), "{no}");
        }
    }

    #[test]
    fn stderr_tail_reads_the_end_of_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("stderr.txt");
        assert_eq!(stderr_tail(&p, 10), "");
        std::fs::write(&p, "0123456789abcdef").unwrap();
        assert_eq!(stderr_tail(&p, 6), "abcdef");
        assert_eq!(stderr_tail(&p, 100), "0123456789abcdef");
        std::fs::write(&p, [0xFFu8, b'o', b'k']).unwrap();
        assert!(stderr_tail(&p, 10).ends_with("ok"));
    }

    #[test]
    fn messages_and_step_names() {
        assert_eq!(
            stop_detail(7, "dev", "deepseek"),
            "Run 7 (dev) trên engine deepseek dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine."
        );
        let hit = QuotaHit {
            signal: "stderr",
            detail: "HTTP 429".to_string(),
            reset_at_ms: None,
        };
        let m = user_message("claude", &hit);
        assert!(
            m.starts_with("Hết quota engine claude: HTTP 429 — agent-brain không tự đổi engine.")
        );
        assert!(m.contains("Chạy lại bước này bằng đội hình"));

        for (stage, role, want) in [
            (Some("plan"), "lead", "Lên plan"),
            (Some("plan-review"), "plan-review", "Review plan"),
            (Some("implement"), "dev", "Code"),
            (Some("code-review"), "code-review", "Review code"),
            (Some("qa"), "qa", "QA"),
            (Some("fix"), "dev", "Sửa"),
            (None, "lead", "Lên plan"),
            (None, "dev", "Code"),
            (None, "qa", "QA"),
            (None, "code-review", "Review code"),
            (None, "plan-review", "Review plan"),
        ] {
            assert_eq!(step_name(stage, role), want, "{stage:?} {role}");
        }
    }
}
