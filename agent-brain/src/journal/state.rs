//! Trạng thái xuất nhật ký (`journal_state`, khóa → giá trị) và hộp thư Telegram
//! (`journal_mail`, gửi đúng một lần nhờ `dedupe_key` UNIQUE). Chính sách lỗi (D-10): đếm số lần
//! đẩy thất bại liên tiếp, báo một lần khi đủ ngưỡng cấu hình, báo phục hồi, và một tin cho mỗi
//! nguyên nhân chặn. Văn bản tin chỉ có loại lỗi, tên luật và đường dẫn tương đối trong vault.

use super::gitsync::{GitFailure, GitStop};
use super::{ExportOutcome, ExportReport};
use crate::error::BrainResult;
use rusqlite::{params, Connection, OptionalExtension};

pub const KEY_LAST_EXPORT_MS: &str = "last_export_ms";
pub const KEY_LAST_EXPORT_DAY: &str = "last_export_day";
pub const KEY_LAST_PUBLISHED_MS: &str = "last_published_ms";
pub const KEY_LAST_PUBLISHED_DAY: &str = "last_published_day";
pub const KEY_LAST_PUSHED_SHA: &str = "last_pushed_sha";
/// Số lần đẩy thất bại liên tiếp.
pub const KEY_PUSH_FAIL_STREAK: &str = "push_fail_streak";
/// Thời điểm (epoch ms) lần thất bại đầu của chuỗi hiện tại; rỗng khi không có chuỗi.
pub const KEY_PUSH_FAIL_SINCE_MS: &str = "push_fail_since_ms";
/// `1` khi đã báo chuỗi thất bại hiện tại, ngược lại `0`.
pub const KEY_PUSH_ALERT_SENT: &str = "push_alert_sent";
/// Rỗng, hoặc loại nguyên nhân đang chặn (`conflict`, `dirty`, `notrepo`, `secret`,
/// `refused`, `failed`).
pub const KEY_BLOCKED: &str = "blocked";
/// Thời điểm (epoch ms) bắt đầu đợt bị chặn hiện tại; rỗng khi không bị chặn. Khóa chống trùng
/// của tin chặn chứa mốc này nên cùng một nguyên nhân được báo một lần mỗi đợt (WR-03); lần
/// xuất thành công kết thúc đợt.
pub const KEY_ALERT_EPISODE_MS: &str = "alert_episode_ms";
/// Như `KEY_ALERT_EPISODE_MS`, cho tệp lạ: đợt kết thúc khi một lần xuất đã ghi không còn tệp lạ.
pub const KEY_FOREIGN_EPISODE_MS: &str = "foreign_episode_ms";
/// Văn bản tin khi một lần xuất trả lỗi (WR-02).
pub const FAILED_MAIL_TEXT: &str = "Nhật ký: xuất lỗi, xem log của serve.";

/// Danh sách đường dẫn trong một tin: tối đa chừng này mục.
const MAX_LISTED: usize = 10;

pub fn get(conn: &Connection, key: &str) -> BrainResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM journal_state WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str, now_ms: i64) -> BrainResult<()> {
    conn.execute(
        "INSERT INTO journal_state (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_ms],
    )?;
    Ok(())
}

/// Xếp một tin vào hộp thư; `false` khi `dedupe_key` đã có (không gửi trùng, kể cả sau restart).
pub fn enqueue_mail(
    conn: &Connection,
    kind: &str,
    dedupe_key: &str,
    text: &str,
    now_ms: i64,
) -> BrainResult<bool> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO journal_mail (kind, dedupe_key, text, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![kind, dedupe_key, text, now_ms],
    )?;
    Ok(n == 1)
}

/// FNV-1a 64 bit, hex chữ thường: khóa chống trùng ổn định cho một nguyên nhân.
fn fnv1a(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Tối đa `MAX_LISTED` mục, phần còn lại thành `… và {k} tệp khác`.
fn capped<S: AsRef<str>>(items: &[S], sep: &str) -> String {
    let shown: Vec<&str> = items.iter().take(MAX_LISTED).map(|s| s.as_ref()).collect();
    let mut out = shown.join(sep);
    if items.len() > MAX_LISTED {
        out.push_str(sep);
        out.push_str(&format!("… và {} tệp khác", items.len() - MAX_LISTED));
    }
    out
}

/// Mốc bắt đầu của đợt lưu ở `key`; chưa có thì đợt bắt đầu lúc `now_ms`.
fn episode(conn: &Connection, key: &str, now_ms: i64) -> BrainResult<String> {
    match get(conn, key)? {
        Some(s) if !s.is_empty() => Ok(s),
        _ => {
            let s = now_ms.to_string();
            set(conn, key, &s, now_ms)?;
            Ok(s)
        }
    }
}

fn get_u64(conn: &Connection, key: &str) -> BrainResult<u64> {
    Ok(get(conn, key)?
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(0))
}

/// Đẩy được (hoặc không có gì để đẩy mà remote với tới): xóa chuỗi lỗi; đã báo lỗi thì báo
/// phục hồi đúng một lần.
fn on_success(conn: &Connection, now_ms: i64) -> BrainResult<()> {
    if get(conn, KEY_PUSH_ALERT_SENT)?.as_deref() == Some("1") {
        let since = get(conn, KEY_PUSH_FAIL_SINCE_MS)?.unwrap_or_default();
        enqueue_mail(
            conn,
            "recovered",
            &format!("recovered:{since}"),
            "Nhật ký: đã đẩy lại được lên repo vault.",
            now_ms,
        )?;
    }
    set(conn, KEY_PUSH_FAIL_STREAK, "0", now_ms)?;
    set(conn, KEY_PUSH_FAIL_SINCE_MS, "", now_ms)?;
    set(conn, KEY_PUSH_ALERT_SENT, "0", now_ms)?;
    set(conn, KEY_BLOCKED, "", now_ms)?;
    set(conn, KEY_ALERT_EPISODE_MS, "", now_ms)?;
    Ok(())
}

/// Một lần đẩy thất bại nữa; báo một lần khi chuỗi chạm `journal.push_fail_notify_after`.
fn on_push_failed(
    conn: &Connection,
    cfg: &super::config::JournalConfig,
    failure: &GitFailure,
    now_ms: i64,
) -> BrainResult<()> {
    let prev = get_u64(conn, KEY_PUSH_FAIL_STREAK)?;
    let streak = prev + 1;
    let since = match get(conn, KEY_PUSH_FAIL_SINCE_MS)? {
        Some(s) if prev > 0 && !s.is_empty() => s,
        _ => now_ms.to_string(),
    };
    set(conn, KEY_PUSH_FAIL_STREAK, &streak.to_string(), now_ms)?;
    set(conn, KEY_PUSH_FAIL_SINCE_MS, &since, now_ms)?;
    let sent = get(conn, KEY_PUSH_ALERT_SENT)?.as_deref() == Some("1");
    if !sent && streak >= u64::from(cfg.push_fail_notify_after) {
        let text = format!(
            "Nhật ký: chưa đẩy được lên repo vault {streak} lần liên tiếp ({}). Commit vẫn nằm trên máy, sẽ tự đẩy bù khi kết nối lại.",
            failure.category.label()
        );
        enqueue_mail(
            conn,
            "pushfail",
            &format!("pushfail:{since}"),
            &text,
            now_ms,
        )?;
        set(conn, KEY_PUSH_ALERT_SENT, "1", now_ms)?;
    }
    Ok(())
}

/// Loại, khóa chống trùng và văn bản tin cho một lần dừng của git.
fn stop_mail(stop: &GitStop, day: &str, ep: &str) -> (&'static str, String, String) {
    match stop {
        GitStop::Conflict { detail } => {
            let paths: Vec<&str> = detail.split(", ").filter(|p| !p.is_empty()).collect();
            (
                "conflict",
                format!("conflict:{ep}:{}", fnv1a(detail)),
                format!(
                    "Nhật ký: xung đột khi lấy thay đổi từ repo vault ({}) — đã dừng, không ghi đè. Sửa xong chạy: agent-brain journal export",
                    capped(&paths, ", ")
                ),
            )
        }
        GitStop::Dirty { paths } => (
            "dirty",
            format!("dirty:{ep}:{}", fnv1a(&paths.join(", "))),
            format!(
                "Nhật ký: bản clone của vault có thay đổi chưa commit ({}) — đã dừng, không ghi đè.",
                capped(paths, ", ")
            ),
        ),
        GitStop::NotARepo => (
            "notrepo",
            format!("notrepo:{day}"),
            "Nhật ký: thư mục clone của vault không phải repo git — đã dừng, không xoá gì."
                .to_string(),
        ),
    }
}

fn record_in(
    conn: &Connection,
    cfg: &super::config::JournalConfig,
    report: &ExportReport,
    now_ms: i64,
) -> BrainResult<()> {
    let day = report.today.format("%Y-%m-%d").to_string();
    let now = now_ms.to_string();
    set(conn, KEY_LAST_EXPORT_MS, &now, now_ms)?;
    set(conn, KEY_LAST_EXPORT_DAY, &day, now_ms)?;
    match &report.outcome {
        ExportOutcome::Pushed { sha } => {
            set(conn, KEY_LAST_PUBLISHED_MS, &now, now_ms)?;
            set(conn, KEY_LAST_PUBLISHED_DAY, &day, now_ms)?;
            set(conn, KEY_LAST_PUSHED_SHA, sha, now_ms)?;
            on_success(conn, now_ms)?;
        }
        ExportOutcome::UpToDate => {
            set(conn, KEY_LAST_PUBLISHED_MS, &now, now_ms)?;
            set(conn, KEY_LAST_PUBLISHED_DAY, &day, now_ms)?;
            on_success(conn, now_ms)?;
        }
        ExportOutcome::PushFailed(f) => on_push_failed(conn, cfg, f, now_ms)?,
        ExportOutcome::Stopped(stop) => {
            let ep = episode(conn, KEY_ALERT_EPISODE_MS, now_ms)?;
            let (kind, key, text) = stop_mail(stop, &day, &ep);
            enqueue_mail(conn, kind, &key, &text, now_ms)?;
            set(conn, KEY_BLOCKED, kind, now_ms)?;
        }
        ExportOutcome::SecretBlocked(hits) => {
            let lines: Vec<String> = hits
                .iter()
                .map(|h| format!("{} ({})", h.path, h.rule))
                .collect();
            let text = format!(
                "Nhật ký: phát hiện chuỗi giống secret — không ghi, không commit, không đẩy.\n{}",
                capped(&lines, "\n")
            );
            let ep = episode(conn, KEY_ALERT_EPISODE_MS, now_ms)?;
            let key = format!("secret:{ep}:{}", fnv1a(&lines.join("\n")));
            enqueue_mail(conn, "secret", &key, &text, now_ms)?;
            set(conn, KEY_BLOCKED, "secret", now_ms)?;
        }
        ExportOutcome::Refused(reason) => {
            let ep = episode(conn, KEY_ALERT_EPISODE_MS, now_ms)?;
            let key = format!("refused:{ep}:{}", fnv1a(reason));
            enqueue_mail(conn, "refused", &key, reason, now_ms)?;
            set(conn, KEY_BLOCKED, "refused", now_ms)?;
        }
        ExportOutcome::Failed => {
            let ep = episode(conn, KEY_ALERT_EPISODE_MS, now_ms)?;
            enqueue_mail(
                conn,
                "failed",
                &format!("failed:{ep}"),
                FAILED_MAIL_TEXT,
                now_ms,
            )?;
            set(conn, KEY_BLOCKED, "failed", now_ms)?;
        }
        ExportOutcome::LocalOnly | ExportOutcome::Busy => {}
    }
    let wrote = matches!(
        report.outcome,
        ExportOutcome::Pushed { .. } | ExportOutcome::UpToDate | ExportOutcome::LocalOnly
    );
    if report.foreign.is_empty() {
        if wrote {
            set(conn, KEY_FOREIGN_EPISODE_MS, "", now_ms)?;
        }
    } else {
        let ep = episode(conn, KEY_FOREIGN_EPISODE_MS, now_ms)?;
        let joined = report.foreign.join(", ");
        let text = format!(
            "Nhật ký: các tệp sau trong vault không có dòng đánh dấu sinh tự động nên không bị ghi đè: {}",
            capped(&report.foreign, ", ")
        );
        enqueue_mail(
            conn,
            "foreign",
            &format!("foreign:{ep}:{}", fnv1a(&joined)),
            &text,
            now_ms,
        )?;
    }
    Ok(())
}

/// Ghi kết quả một lần xuất: mọi kết quả trừ `Busy` cập nhật `last_export_*`; `Pushed` và
/// `UpToDate` cập nhật `last_published_*` (`Pushed` ghi thêm `last_pushed_sha`) và xóa chuỗi
/// lỗi và kết thúc đợt bị chặn; `PushFailed` đếm chuỗi lỗi; `Stopped`/`SecretBlocked`/
/// `Refused`/`Failed` và tệp lạ xếp một tin cho mỗi nguyên nhân trong mỗi đợt (WR-03). Mọi
/// thay đổi của một lần ghi nằm trong một giao dịch.
pub fn record_outcome(
    conn: &Connection,
    cfg: &super::config::JournalConfig,
    report: &ExportReport,
    now_ms: i64,
) -> BrainResult<()> {
    if report.outcome == ExportOutcome::Busy {
        return Ok(());
    }
    if conn.is_autocommit() {
        let tx = conn.unchecked_transaction()?;
        record_in(&tx, cfg, report, now_ms)?;
        tx.commit()?;
        Ok(())
    } else {
        record_in(conn, cfg, report, now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::config::JournalConfig;
    use crate::journal::gitsync::{FailCategory, GitFailure};
    use chrono::NaiveDate;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        (dir, conn)
    }

    fn report(outcome: ExportOutcome, day: u32) -> ExportReport {
        ExportReport {
            outcome,
            today: NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
            today_summary: None,
            changed: Vec::new(),
            removed: Vec::new(),
            foreign: Vec::new(),
        }
    }

    #[test]
    fn record_outcome_sets_export_keys() {
        let (_d, conn) = db();
        let cfg = JournalConfig::default();
        let pushed = report(
            ExportOutcome::Pushed {
                sha: "abc123".to_string(),
            },
            15,
        );
        record_outcome(&conn, &cfg, &pushed, 1000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-15")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_DAY).unwrap().as_deref(),
            Some("2026-01-15")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUSHED_SHA).unwrap().as_deref(),
            Some("abc123")
        );

        let failed = report(
            ExportOutcome::PushFailed(GitFailure {
                category: FailCategory::Network,
                summary: FailCategory::Network.label().to_string(),
            }),
            16,
        );
        record_outcome(&conn, &cfg, &failed, 2000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("2000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-16")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUSHED_SHA).unwrap().as_deref(),
            Some("abc123")
        );

        let busy = report(ExportOutcome::Busy, 17);
        record_outcome(&conn, &cfg, &busy, 3000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("2000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-16")
        );
    }

    fn cfg3() -> JournalConfig {
        JournalConfig {
            push_fail_notify_after: 3,
            ..JournalConfig::default()
        }
    }

    fn fail() -> ExportReport {
        report(
            ExportOutcome::PushFailed(GitFailure {
                category: FailCategory::Network,
                summary: FailCategory::Network.label().to_string(),
            }),
            15,
        )
    }

    fn pushed() -> ExportReport {
        report(
            ExportOutcome::Pushed {
                sha: "abc".to_string(),
            },
            15,
        )
    }

    fn mails(conn: &Connection, kind: &str) -> Vec<(String, String)> {
        conn.prepare("SELECT dedupe_key, text FROM journal_mail WHERE kind = ?1 ORDER BY id")
            .unwrap()
            .query_map(params![kind], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    fn val(conn: &Connection, key: &str) -> String {
        get(conn, key).unwrap().unwrap_or_default()
    }

    #[test]
    fn alert_once_after_n_failures() {
        let (_d, conn) = db();
        let cfg = cfg3();
        record_outcome(&conn, &cfg, &fail(), 100).unwrap();
        record_outcome(&conn, &cfg, &fail(), 200).unwrap();
        assert!(mails(&conn, "pushfail").is_empty());
        record_outcome(&conn, &cfg, &fail(), 300).unwrap();
        let rows = mails(&conn, "pushfail");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, "pushfail:100");
        assert!(rows[0].1.contains("3 lần liên tiếp"), "{}", rows[0].1);
        assert!(rows[0].1.contains(FailCategory::Network.label()));
        record_outcome(&conn, &cfg, &fail(), 400).unwrap();
        record_outcome(&conn, &cfg, &fail(), 500).unwrap();
        assert_eq!(mails(&conn, "pushfail").len(), 1);
        assert_eq!(val(&conn, KEY_PUSH_ALERT_SENT), "1");
        assert_eq!(val(&conn, KEY_PUSH_FAIL_STREAK), "5");
        assert_eq!(val(&conn, KEY_PUSH_FAIL_SINCE_MS), "100");
    }

    #[test]
    fn recovery_mail_after_alert() {
        let (_d, conn) = db();
        let cfg = cfg3();
        for t in [100, 200, 300] {
            record_outcome(&conn, &cfg, &fail(), t).unwrap();
        }
        record_outcome(&conn, &cfg, &pushed(), 400).unwrap();
        let rec = mails(&conn, "recovered");
        assert_eq!(rec.len(), 1, "{rec:?}");
        assert_eq!(rec[0].0, "recovered:100");
        assert_eq!(rec[0].1, "Nhật ký: đã đẩy lại được lên repo vault.");
        assert_eq!(val(&conn, KEY_PUSH_FAIL_STREAK), "0");
        assert_eq!(val(&conn, KEY_PUSH_ALERT_SENT), "0");

        record_outcome(&conn, &cfg, &pushed(), 450).unwrap();
        assert_eq!(mails(&conn, "recovered").len(), 1);
        for t in [500, 600, 700] {
            record_outcome(&conn, &cfg, &fail(), t).unwrap();
        }
        let alerts = mails(&conn, "pushfail");
        assert_eq!(alerts.len(), 2, "{alerts:?}");
        assert_eq!(alerts[1].0, "pushfail:500");
        record_outcome(&conn, &cfg, &pushed(), 800).unwrap();
        assert_eq!(mails(&conn, "recovered").len(), 2);
    }

    #[test]
    fn no_recovery_mail_without_alert() {
        let (_d, conn) = db();
        let cfg = cfg3();
        record_outcome(&conn, &cfg, &fail(), 100).unwrap();
        record_outcome(&conn, &cfg, &fail(), 200).unwrap();
        record_outcome(&conn, &cfg, &report(ExportOutcome::UpToDate, 15), 300).unwrap();
        assert!(mails(&conn, "recovered").is_empty());
        assert!(mails(&conn, "pushfail").is_empty());
        assert_eq!(val(&conn, KEY_PUSH_FAIL_STREAK), "0");
    }

    #[test]
    fn cause_mails_once_per_cause() {
        let (_d, conn) = db();
        let cfg = cfg3();
        let conflict = |d: &str| {
            report(
                ExportOutcome::Stopped(GitStop::Conflict {
                    detail: d.to_string(),
                }),
                15,
            )
        };
        record_outcome(&conn, &cfg, &conflict("Today.md"), 100).unwrap();
        record_outcome(&conn, &cfg, &conflict("Today.md"), 200).unwrap();
        let rows = mails(&conn, "conflict");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!(rows[0].0.starts_with("conflict:100:") && rows[0].0.len() == 13 + 16);
        assert!(rows[0].1.contains("(Today.md)"), "{}", rows[0].1);
        assert!(rows[0].1.contains("agent-brain journal export"));
        assert_eq!(val(&conn, KEY_BLOCKED), "conflict");
        record_outcome(&conn, &cfg, &conflict("Nhật ký/2026/01/2026-01-15.md"), 300).unwrap();
        assert_eq!(mails(&conn, "conflict").len(), 2);

        let many: Vec<String> = (0..12).map(|i| format!("Nhật ký/a-{i:02}.md")).collect();
        let dirty = report(ExportOutcome::Stopped(GitStop::Dirty { paths: many }), 15);
        record_outcome(&conn, &cfg, &dirty, 400).unwrap();
        record_outcome(&conn, &cfg, &dirty, 410).unwrap();
        let rows = mails(&conn, "dirty");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!(rows[0].1.contains("Nhật ký/a-09.md"), "{}", rows[0].1);
        assert!(!rows[0].1.contains("Nhật ký/a-10.md"), "{}", rows[0].1);
        assert!(rows[0].1.contains("… và 2 tệp khác"), "{}", rows[0].1);
        assert_eq!(val(&conn, KEY_BLOCKED), "dirty");

        let notrepo = report(ExportOutcome::Stopped(GitStop::NotARepo), 15);
        record_outcome(&conn, &cfg, &notrepo, 500).unwrap();
        record_outcome(&conn, &cfg, &notrepo, 510).unwrap();
        let rows = mails(&conn, "notrepo");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "notrepo:2026-01-15");
        assert_eq!(val(&conn, KEY_BLOCKED), "notrepo");

        record_outcome(&conn, &cfg, &report(ExportOutcome::UpToDate, 15), 600).unwrap();
        assert_eq!(val(&conn, KEY_BLOCKED), "");
        record_outcome(&conn, &cfg, &conflict("Today.md"), 700).unwrap();
        assert_eq!(val(&conn, KEY_BLOCKED), "conflict");
        record_outcome(&conn, &cfg, &pushed(), 800).unwrap();
        assert_eq!(val(&conn, KEY_BLOCKED), "");
        // Stops are not push failures.
        assert_eq!(val(&conn, KEY_PUSH_FAIL_STREAK), "0");
    }

    #[test]
    fn cause_mail_recurs_in_new_episode() {
        let (_d, conn) = db();
        let cfg = cfg3();
        let conflict = report(
            ExportOutcome::Stopped(GitStop::Conflict {
                detail: "Today.md".to_string(),
            }),
            15,
        );
        let refused = report(ExportOutcome::Refused("lý do".to_string()), 15);
        let failed = report(ExportOutcome::Failed, 15);
        for t in [100, 200] {
            record_outcome(&conn, &cfg, &conflict, t).unwrap();
            record_outcome(&conn, &cfg, &refused, t + 1).unwrap();
            record_outcome(&conn, &cfg, &failed, t + 2).unwrap();
        }
        assert_eq!(mails(&conn, "conflict").len(), 1);
        assert_eq!(mails(&conn, "refused").len(), 1);
        assert_eq!(mails(&conn, "failed").len(), 1);
        assert_eq!(val(&conn, KEY_BLOCKED), "failed");
        assert_eq!(val(&conn, KEY_LAST_EXPORT_MS), "202");
        record_outcome(&conn, &cfg, &pushed(), 300).unwrap();
        assert_eq!(val(&conn, KEY_ALERT_EPISODE_MS), "");
        for t in [400, 500] {
            record_outcome(&conn, &cfg, &conflict, t).unwrap();
            record_outcome(&conn, &cfg, &failed, t + 2).unwrap();
        }
        let rows = mails(&conn, "conflict");
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows[1].0.starts_with("conflict:400:"), "{rows:?}");
        assert_eq!(mails(&conn, "failed").len(), 2);
        assert_eq!(mails(&conn, "failed")[0].1, FAILED_MAIL_TEXT);
    }

    #[test]
    fn foreign_mail_recurs_after_clean_export() {
        let (_d, conn) = db();
        let cfg = cfg3();
        let mut r = pushed();
        r.foreign = vec!["Today.md".to_string()];
        record_outcome(&conn, &cfg, &r, 100).unwrap();
        // Stopped exports never reach the writer: they do not end the foreign episode.
        let stop = report(ExportOutcome::Stopped(GitStop::NotARepo), 15);
        record_outcome(&conn, &cfg, &stop, 150).unwrap();
        record_outcome(&conn, &cfg, &r, 200).unwrap();
        assert_eq!(mails(&conn, "foreign").len(), 1);
        record_outcome(&conn, &cfg, &pushed(), 300).unwrap();
        record_outcome(&conn, &cfg, &r, 400).unwrap();
        assert_eq!(mails(&conn, "foreign").len(), 2);
    }

    #[test]
    fn secret_mail_has_paths_not_values() {
        let (_d, conn) = db();
        let cfg = cfg3();
        let hit = crate::journal::sanitize::SecretHit {
            path: "Dự án/[đã ẩn].md".to_string(),
            rule: "known-value".to_string(),
        };
        let r = report(ExportOutcome::SecretBlocked(vec![hit]), 15);
        record_outcome(&conn, &cfg, &r, 100).unwrap();
        record_outcome(&conn, &cfg, &r, 200).unwrap();
        let rows = mails(&conn, "secret");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!(
            rows[0].1.contains("Dự án/[đã ẩn].md (known-value)"),
            "{}",
            rows[0].1
        );
        assert!(rows[0].1.contains("không commit"), "{}", rows[0].1);
        assert_eq!(val(&conn, KEY_BLOCKED), "secret");
    }

    #[test]
    fn foreign_mail_dedupes() {
        let (_d, conn) = db();
        let cfg = cfg3();
        let mut r = pushed();
        r.foreign = vec!["Today.md".to_string()];
        record_outcome(&conn, &cfg, &r, 100).unwrap();
        record_outcome(&conn, &cfg, &r, 200).unwrap();
        let rows = mails(&conn, "foreign");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!(rows[0].1.ends_with(": Today.md"), "{}", rows[0].1);
    }

    #[test]
    fn state_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brain.db");
        let cfg = cfg3();
        {
            let conn = crate::db::open(&path).unwrap();
            record_outcome(&conn, &cfg, &fail(), 100).unwrap();
            record_outcome(&conn, &cfg, &fail(), 200).unwrap();
            assert!(mails(&conn, "pushfail").is_empty());
        }
        let conn = crate::db::open(&path).unwrap();
        record_outcome(&conn, &cfg, &fail(), 300).unwrap();
        let rows = mails(&conn, "pushfail");
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, "pushfail:100");
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(""), "cbf29ce484222325");
        assert_eq!(fnv1a("a"), "af63dc4c8601ec8c");
    }

    #[test]
    fn enqueue_mail_dedupes() {
        let (_d, conn) = db();
        assert!(enqueue_mail(&conn, "digest", "digest:2026-01-15", "a", 1).unwrap());
        assert!(!enqueue_mail(&conn, "digest", "digest:2026-01-15", "b", 2).unwrap());
        let n: i64 = conn
            .query_row("SELECT count(*) FROM journal_mail", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
