//! Message texts and inline buttons (Phase 05.3, D-03/D-11/D-12/D-16).
//!
//! Every text is plain Vietnamese sent without a formatting mode, so task
//! titles, plan titles and file names (written by agents) cannot inject markup.
//! Callback data is built only by `callback::encode` from DB ids and the rev.

use super::callback::{self, Callback, Op, TeamCb, TeamOp};
use super::detect::{Event, EventKind};
use crate::autopilot::decide::StopReason;
use crate::dashboard::api_tasks::TaskState;
use crate::db::RunRow;

pub const MAX_TEXT: usize = 3900;
/// Telegram's limit for the text of one message.
pub const TELEGRAM_MAX: usize = 4096;
/// Longest suffix `with_suffix` keeps whole; a longer one is clipped.
const SUFFIX_MAX: usize = 1000;
pub const MAX_FILES: usize = 15;
/// Gate line of a task created from Telegram: its plan is approved on the dashboard (05.4 INTAKE-07).
pub const TELEGRAM_ORIGIN_LINE: &str = "Task giao từ Telegram — duyệt plan trên dashboard.";

#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub label: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub text: String,
    pub keyboard: Vec<Vec<Button>>,
    /// `(file name, content)` of the plan document sent before the text (gate only).
    pub document: Option<(String, String)>,
}

/// Cuts `text` to at most `max` characters, ending with `\n…` when cut.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max < 2 {
        return text.chars().take(max).collect();
    }
    let mut out: String = text.chars().take(max - 2).collect();
    out.push_str("\n…");
    out
}

/// `original` with `suffix` appended, the whole text within `TELEGRAM_MAX`
/// characters: the suffix is kept (up to `SUFFIX_MAX`) and the original is
/// clipped to what is left, so an edit never fails with "message is too long".
pub fn with_suffix(original: &str, suffix: &str) -> String {
    let suffix = clip(suffix, SUFFIX_MAX);
    let room = TELEGRAM_MAX
        .saturating_sub(suffix.chars().count())
        .min(MAX_TEXT);
    format!("{}{suffix}", clip(original, room))
}

fn running_run(s: &TaskState) -> Option<&RunRow> {
    s.runs
        .iter()
        .filter(|r| r.status == "running")
        .max_by_key(|r| (r.started_at, r.id))
}

fn latest_run(s: &TaskState) -> Option<&RunRow> {
    s.runs.iter().max_by_key(|r| (r.started_at, r.id))
}

fn has_control(s: &TaskState, id: &str) -> bool {
    s.view.controls.iter().any(|c| c.id == id)
}

/// True when the task was created from Telegram (`tasks.origin = telegram`).
pub fn is_telegram_origin(s: &TaskState) -> bool {
    s.origin.as_deref() == Some(crate::db::ORIGIN_TELEGRAM)
}

/// `Duyệt plan` is offered only when the task allows it and it was not created
/// from Telegram. Hiding is not the guard (the shared approve action refuses a
/// Telegram approval too); it just avoids a button that can only be refused.
fn approve_offered(s: &TaskState) -> bool {
    has_control(s, "approve") && !is_telegram_origin(s)
}

fn has_action(s: &TaskState, id: &str) -> bool {
    s.view.actions.iter().any(|a| a.id == id)
}

/// `Dự án: {project} · profile {profile} · nhánh {branch}` (D-16).
pub fn project_line(s: &TaskState) -> String {
    format!(
        "Dự án: {} · profile {} · nhánh {}",
        s.project.id, s.project.profile, s.task.branch
    )
}

/// Longest task title line shown in a notification.
const TITLE_MAX: usize = 120;

/// The task title as one line (05.4 Pitfall 8): the first line, at most
/// `TITLE_MAX` characters, `…` when cut. A long forwarded request cannot push
/// the plan section out of the message.
pub fn title_line(s: &TaskState) -> String {
    title_line_of(&s.task.title)
}

/// Pure part of `title_line`.
pub fn title_line_of(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim_end();
    if first.chars().count() <= TITLE_MAX {
        return first.to_string();
    }
    let mut out: String = first.chars().take(TITLE_MAX - 1).collect();
    out.push('…');
    out
}

/// Verdict and round of the newest plan review.
pub fn plan_review_line(s: &TaskState) -> String {
    let newest = s
        .runs
        .iter()
        .filter(|r| r.role == "plan-review")
        .max_by_key(|r| (r.started_at, r.id));
    match newest {
        Some(r) => {
            let word = match r.outcome.as_deref() {
                Some("review:approve") => "duyệt",
                Some("review:revise") => "sửa lại",
                _ => crate::dashboard::taskstate::step_state(Some(r)).1,
            };
            let round = r.round.unwrap_or(1);
            match &s.workflow {
                Some(wf) => format!("Review plan: {word} · vòng {round} / {}", wf.max_rounds),
                None => format!("Review plan: {word} · vòng {round}"),
            }
        }
        None => match &s.workflow {
            Some(wf) if wf.has("plan-review") => "Review plan: chưa chạy".to_string(),
            Some(wf) => format!("Review plan: không có trong quy trình {}", wf.name),
            None => "Review plan: không có trong quy trình".to_string(),
        },
    }
}

/// `Tổng: {n} run · {m} phút · ${cost}` over every run of the task.
pub fn summary_line(s: &TaskState, now_ms: i64) -> String {
    let count = s.runs.len();
    let total_ms: i64 = s
        .runs
        .iter()
        .map(|r| (r.ended_at.unwrap_or(now_ms) - r.started_at).max(0))
        .sum();
    let cost: f64 = s.runs.iter().map(|r| r.cost_usd).sum();
    format!(
        "Tổng: {count} run · {} phút · {}",
        total_ms / 60_000,
        crate::autopilot::view::fmt_usd(cost)
    )
}

/// One short line describing where the task is now.
pub fn state_line(s: &TaskState) -> String {
    let auto = s.view.auto.state.as_str();
    if s.driver_alive || auto == "running" || auto == "stopping" {
        return "Autopilot đang chạy".to_string();
    }
    if let Some(r) = running_run(s) {
        return format!("Run {} ({}) đang chạy", r.id, r.role);
    }
    if let Some(b) = &s.view.banner {
        return b.clone();
    }
    if s.task.status == "open" {
        return "Chưa có plan".to_string();
    }
    s.task.status.clone()
}

/// Result line appended to an acted-on message (D-15: who acted and when).
pub fn result_suffix(what: &str, via_telegram: bool, at_ms: i64) -> String {
    let hhmm = crate::autopilot::view::fmt_hhmm(at_ms);
    if via_telegram {
        format!("\n\n✓ {what} — từ Telegram lúc {hhmm}")
    } else {
        format!("\n\n{what} — xử lý ngoài Telegram (dashboard hoặc terminal) lúc {hhmm}")
    }
}

fn slug_ok(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn plan_lines(s: &TaskState, lines: &mut Vec<String>) {
    match &s.plan_text {
        Some(text) => {
            let plan = crate::planfile::parse_plan(text);
            let title = if plan.title.trim().is_empty() {
                "(không có tiêu đề)".to_string()
            } else {
                plan.title.clone()
            };
            lines.push(format!("Plan: {title}"));
            let files = crate::planfile::plan_files(&plan);
            lines.push(format!("Thay đổi ({} file):", files.len()));
            for f in files.iter().take(MAX_FILES) {
                lines.push(format!("- {f}"));
            }
            if files.len() > MAX_FILES {
                lines.push(format!("… và {} file khác", files.len() - MAX_FILES));
            }
        }
        None => lines.push("Plan: (không đọc được .plan.md)".to_string()),
    }
}

fn failed_run<'a>(ev: &Event, s: &'a TaskState) -> Option<&'a RunRow> {
    ev.key
        .strip_prefix("fail:")
        .and_then(|id| id.parse::<i64>().ok())
        .and_then(|id| s.runs.iter().find(|r| r.id == id))
        .or_else(|| latest_run(s))
}

/// The quota run a `Quota` event names (`quota:{run id}`), else the one the task state calls for.
pub fn quota_run_of<'a>(ev: &Event, s: &'a TaskState) -> Option<&'a RunRow> {
    ev.key
        .strip_prefix("quota:")
        .and_then(|id| id.parse::<i64>().ok())
        .and_then(|id| s.runs.iter().find(|r| r.id == id))
        .or_else(|| super::detect::quota_run(s))
}

/// Vietnamese name of the step the quota run belonged to (`Code`, `QA`, …).
pub fn quota_step_name(r: &RunRow) -> &'static str {
    crate::quota::step_name(r.stage.as_deref(), &r.role)
}

/// The question of the confirmation a picked team opens (D-28): nothing runs before `Chạy lại bước này`.
pub fn rerun_confirm_text(step: &str, team: &str, engine: &str) -> String {
    format!("Chạy lại bước {step} bằng đội hình {team}?\nBước {step} sẽ chạy bằng: {engine}")
}

/// The line that replaces the confirmation once the shared re-run started a job.
pub fn rerun_done_text(team: &str, step: &str, job_id: u64) -> String {
    format!("Đã đổi sang đội hình {team} và giao lại bước {step} (job {job_id}).")
}

/// Added under the quota message when the user chooses `Thử lại sau`; nothing is written.
pub const RETRY_LATER_LINE: &str =
    "Đã hoãn — khi quota hồi phục, chạy lại bước này trên dashboard.";

/// The team names a quota message offers: the first `MAX_TEAM_BUTTONS` of the list the server
/// holds now. A button carries an index into this list and the version of it.
pub fn team_names(s: &TaskState) -> Vec<String> {
    s.teams
        .names()
        .into_iter()
        .take(callback::MAX_TEAM_BUTTONS)
        .collect()
}

fn text_for(ev: &Event, s: &TaskState, now_ms: i64) -> String {
    let id = s.task.id;
    let mut lines: Vec<String> = Vec::new();
    match ev.kind {
        EventKind::Gate => {
            lines.push(format!("Chờ duyệt plan · task {id}"));
            lines.push(title_line(s));
            lines.push(project_line(s));
            if is_telegram_origin(s) {
                lines.push(TELEGRAM_ORIGIN_LINE.to_string());
            }
            plan_lines(s, &mut lines);
            lines.push(plan_review_line(s));
            if let Some(r) = running_run(s) {
                lines.push(format!("Đang có run {} ({}) chạy.", r.id, r.role));
            }
        }
        EventKind::Stop => {
            let stop = s.aj.stop.as_ref();
            let word = s
                .view
                .auto
                .stop_word
                .clone()
                .or_else(|| stop.map(|i| crate::autopilot::view::reason_word(i.reason).to_string()))
                .unwrap_or_default();
            lines.push(format!("Autopilot dừng · {word} · task {id}"));
            lines.push(title_line(s));
            lines.push(project_line(s));
            let detail = s
                .view
                .auto
                .stop_detail
                .clone()
                .or_else(|| stop.map(|i| crate::autopilot::view::stop_detail(i, &s.task.slug)));
            if let Some(d) = detail {
                lines.push(d);
            }
            if stop.map(|i| i.reason) == Some(StopReason::HetNganSach) {
                lines.push("Chạy vượt ngân sách một lần, chỉ cho vòng này.".to_string());
            }
        }
        EventKind::QaPass => {
            lines.push(format!("QA đạt · task {id}"));
            lines.push(title_line(s));
            lines.push(project_line(s));
            lines.push(summary_line(s, now_ms));
        }
        EventKind::Quota => {
            if let Some(r) = quota_run_of(ev, s) {
                lines.push(format!(
                    "Hết quota engine {} · task {id} {}",
                    r.engine, s.task.slug
                ));
                lines.push(crate::quota::stop_detail(r.id, &r.role, &r.engine));
                lines.push(format!(
                    "Chọn đội hình để chạy lại bước {} (engine mới tiếp tục từ thư mục làm việc hiện tại) hoặc Thử lại sau.",
                    quota_step_name(r)
                ));
            } else {
                lines.push(format!("Hết quota · task {id} {}", s.task.slug));
            }
            let hidden = s
                .teams
                .teams
                .len()
                .saturating_sub(callback::MAX_TEAM_BUTTONS);
            if hidden > 0 {
                lines.push(format!("Còn {hidden} đội hình khác — chọn trên dashboard."));
            }
        }
        EventKind::RunFailed => {
            lines.push(format!("Run lỗi · task {id}"));
            lines.push(title_line(s));
            lines.push(project_line(s));
            if let Some(r) = failed_run(ev, s) {
                lines.push(format!(
                    "Run {} ({}) lỗi hoặc quá thời gian — xem log của run này.",
                    r.id, r.role
                ));
            }
            lines.push(summary_line(s, now_ms));
        }
    }
    clip(&lines.join("\n"), MAX_TEXT)
}

fn button(label: &str, op: Op, s: &TaskState, run_id: Option<i64>) -> Vec<Button> {
    vec![Button {
        label: label.to_string(),
        data: callback::encode(&Callback {
            op,
            task_id: s.task.id,
            run_id,
            rev: s.rev.clone(),
        }),
    }]
}

/// Buttons the task's current state allows for this event (one per row).
pub fn buttons_for(ev: &Event, s: &TaskState) -> Vec<Vec<Button>> {
    let mut rows = Vec::new();
    let return_lead = has_control(s, "return_lead") || has_action(s, "relead");
    match ev.kind {
        EventKind::Gate => {
            if approve_offered(s) {
                rows.push(button("Duyệt plan", Op::Approve, s, None));
            }
            if has_control(s, "return_lead") {
                rows.push(button("Trả về Lead", Op::ReturnLead, s, None));
            }
        }
        EventKind::Stop => {
            if has_control(s, "auto_more_round") {
                rows.push(button("Chạy thêm 1 vòng", Op::MoreRound, s, None));
            }
            if return_lead {
                rows.push(button("Trả về Lead", Op::ReturnLead, s, None));
            }
            if has_control(s, "auto_end") {
                rows.push(button("Dừng hẳn", Op::End, s, None));
            }
        }
        EventKind::RunFailed => {
            if approve_offered(s) {
                rows.push(button("Duyệt plan", Op::Approve, s, None));
            }
            if return_lead {
                rows.push(button("Trả về Lead", Op::ReturnLead, s, None));
            }
        }
        EventKind::QaPass => {}
        EventKind::Quota => {
            let names = team_names(s);
            let ver = callback::team_list_ver(&names);
            for (idx, name) in names.iter().enumerate() {
                rows.push(vec![Button {
                    label: format!("Đội hình {name}"),
                    data: callback::encode_team(&TeamCb {
                        op: TeamOp::Pick,
                        task_id: s.task.id,
                        idx,
                        ver: ver.clone(),
                        rev: s.rev.clone(),
                    }),
                }]);
            }
            rows.push(button("Thử lại sau", Op::RetryLater, s, None));
        }
    }
    if let Some(r) = running_run(s) {
        rows.push(button("Kill", Op::KillAsk, s, Some(r.id)));
    }
    rows
}

pub fn message_for(ev: &Event, s: &TaskState) -> Rendered {
    let now = crate::db::now_ms();
    let document = match (ev.kind, &s.plan_text) {
        (EventKind::Gate, Some(text)) => {
            let name = if slug_ok(&s.task.slug) {
                format!("{}.plan.md", s.task.slug)
            } else {
                "plan.md".to_string()
            };
            Some((name, text.clone()))
        }
        _ => None,
    };
    Rendered {
        text: text_for(ev, s, now),
        keyboard: buttons_for(ev, s),
        document,
    }
}

pub fn keyboard_json(rows: &[Vec<Button>]) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::Value::Array(
                row.iter()
                    .map(|b| serde_json::json!({"text": b.label, "callback_data": b.data}))
                    .collect(),
            )
        })
        .collect();
    serde_json::json!({ "inline_keyboard": rows })
}

pub fn no_keyboard() -> serde_json::Value {
    serde_json::json!({ "inline_keyboard": [] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_suffix_stays_within_telegram_limit() {
        let original = "ừ".repeat(MAX_TEXT);
        for suffix_len in [0usize, 50, 196, 600, 1000, 5000] {
            let suffix = format!("\n\n{}", "ă".repeat(suffix_len));
            let out = with_suffix(&original, &suffix);
            assert!(
                out.chars().count() <= TELEGRAM_MAX,
                "suffix {suffix_len}: {} chars",
                out.chars().count()
            );
            assert!(out.starts_with('ừ'));
        }
        // A short suffix leaves the original whole and ends the text.
        let out = with_suffix("Chờ duyệt plan", "\n\n✓ xong");
        assert_eq!(out, "Chờ duyệt plan\n\n✓ xong");
        // The suffix survives (clipped only past SUFFIX_MAX) when the original is cut.
        let out = with_suffix(&original, "\n\n✓ Đã trả về Lead (ghi chú: «a»)");
        assert!(out.ends_with("✓ Đã trả về Lead (ghi chú: «a»)"), "{out}");
    }

    #[test]
    fn clip_counts_characters() {
        assert_eq!(clip("ừ ừ", 10), "ừ ừ");
        let long = "đ".repeat(20);
        let cut = clip(&long, 10);
        assert_eq!(cut.chars().count(), 10);
        assert!(cut.ends_with("\n…"));
        assert!(cut.starts_with("đđđđđđđđ\n"));
    }

    #[test]
    fn slug_check() {
        assert!(slug_ok("sua-app-2"));
        assert!(!slug_ok("Sua"));
        assert!(!slug_ok("a/b"));
        assert!(!slug_ok(""));
    }

    #[test]
    fn keyboards() {
        assert_eq!(no_keyboard(), serde_json::json!({"inline_keyboard": []}));
        let rows = vec![vec![Button {
            label: "Duyệt plan".to_string(),
            data: "a:1:0123456789abcdef".to_string(),
        }]];
        assert_eq!(
            keyboard_json(&rows),
            serde_json::json!({"inline_keyboard": [[{"text": "Duyệt plan", "callback_data": "a:1:0123456789abcdef"}]]})
        );
    }

    #[test]
    fn title_line_of_one_line_and_cut() {
        assert_eq!(title_line_of("sửa app"), "sửa app");
        assert_eq!(title_line_of("dòng một\ndòng hai"), "dòng một");
        let exact = "ă".repeat(120);
        assert_eq!(title_line_of(&exact), exact);
        let cut = title_line_of(&"ă".repeat(121));
        assert_eq!(cut, format!("{}…", "ă".repeat(119)));
        assert_eq!(cut.chars().count(), 120);
    }

    #[test]
    fn result_suffix_says_who_acted() {
        let a = result_suffix("Đã duyệt plan", true, 0);
        assert!(
            a.starts_with("\n\n✓ Đã duyệt plan — từ Telegram lúc "),
            "{a}"
        );
        let b = result_suffix("Đã duyệt plan", false, 0);
        assert!(
            b.starts_with(
                "\n\nĐã duyệt plan — xử lý ngoài Telegram (dashboard hoặc terminal) lúc "
            ),
            "{b}"
        );
    }
}
