//! Lệnh fixbug từ Telegram (Plan fixbug-trial A8): `/fixbugs`, `/bugs`, `/deployed`, `/reject`.
//!
//! Chỉ cho dự án trong `telegram_projects` (cùng `intake::allowed_projects` như `/task`),
//! tên dự án khớp như `routing::resolve`. Lệnh gọi Trello (`/fixbugs`, `/deployed`,
//! `/reject`) chạy trên một luồng riêng để không treo vòng poll; mỗi lúc chỉ một lệnh như
//! vậy chạy. Câu trả lời không chứa key, token hay board id (redact lần cuối trước khi gửi).

use super::bot;
use super::intake::allowed_projects;
use super::render::{clip, MAX_TEXT};
use super::BotCtx;
use crate::fixbug::pull::DEFAULT_MAX_NEW;
use crate::projects::{load_map, Project};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Trần số bug một lần `/fixbugs`.
pub const MAX_PULL_TELEGRAM: usize = 20;
/// Số bug mới liệt kê tối đa trong câu trả lời `/fixbugs`.
const MAX_NEW_LINES: usize = 20;
/// Số cảnh báo liệt kê tối đa.
const MAX_WARNING_LINES: usize = 5;
/// Số dòng tối đa của `/bugs`.
const MAX_PROGRESS_LINES: usize = 30;

pub const USAGE_FIXBUGS: &str = "Cách dùng: /fixbugs <dự án> [số bug tối đa, 1–20]";
pub const USAGE_BUGS: &str = "Cách dùng: /bugs <dự án>";
pub const USAGE_DEPLOYED: &str = "Cách dùng: /deployed <dự án> <KEY...> [-- ghi chú]";
pub const USAGE_REJECT: &str = "Cách dùng: /reject <dự án> <KEY> <lý do>";
pub const BUSY: &str = "Đang chạy một lệnh fixbug khác — thử lại sau.";

static RUNNING: AtomicBool = AtomicBool::new(false);

/// `/fixbugs <dự án> [số]` → (dự án, số tối đa). Mặc định `DEFAULT_MAX_NEW`, trần 20.
pub fn parse_fixbug_cmd(rest: &str) -> Result<(String, usize), String> {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    match toks.as_slice() {
        [project] => Ok((project.to_string(), DEFAULT_MAX_NEW.min(MAX_PULL_TELEGRAM))),
        [project, n] => match n.parse::<usize>() {
            Ok(v) if v > 0 => Ok((project.to_string(), v.min(MAX_PULL_TELEGRAM))),
            _ => Err(USAGE_FIXBUGS.to_string()),
        },
        _ => Err(USAGE_FIXBUGS.to_string()),
    }
}

/// `/bugs <dự án>` → dự án.
pub fn parse_bugs_args(rest: &str) -> Result<String, String> {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    match toks.as_slice() {
        [project] => Ok(project.to_string()),
        _ => Err(USAGE_BUGS.to_string()),
    }
}

/// `/deployed <dự án> <KEY...> [-- ghi chú]` → (dự án, các KEY, ghi chú).
pub fn parse_deployed_args(rest: &str) -> Result<(String, Vec<String>, Option<String>), String> {
    let (head, note) = match split_note(rest) {
        Some((h, n)) => (h, Some(n)),
        None => (rest, None),
    };
    let toks: Vec<&str> = head.split_whitespace().collect();
    if toks.len() < 2 {
        return Err(USAGE_DEPLOYED.to_string());
    }
    let note = note
        .map(|n| n.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|n| !n.is_empty());
    Ok((
        toks[0].to_string(),
        toks[1..].iter().map(|k| k.to_string()).collect(),
        note,
    ))
}

/// Tách `head -- note` tại token `--` đầu tiên.
fn split_note(rest: &str) -> Option<(&str, &str)> {
    let mut search = 0;
    while let Some(pos) = rest[search..].find("--") {
        let at = search + pos;
        let before_ok = at == 0 || rest[..at].ends_with(char::is_whitespace);
        let after = &rest[at + 2..];
        let after_ok = after.is_empty() || after.starts_with(char::is_whitespace);
        if before_ok && after_ok {
            return Some((&rest[..at], after));
        }
        search = at + 2;
    }
    None
}

/// `/reject <dự án> <KEY> <lý do>` → (dự án, KEY, lý do). Thiếu lý do ⇒ hướng dẫn.
pub fn parse_reject_args(rest: &str) -> Result<(String, String, String), String> {
    let mut toks = rest.split_whitespace();
    let (Some(project), Some(key)) = (toks.next(), toks.next()) else {
        return Err(USAGE_REJECT.to_string());
    };
    let reason = toks.collect::<Vec<_>>().join(" ");
    if reason.is_empty() {
        return Err(USAGE_REJECT.to_string());
    }
    Ok((project.to_string(), key.to_string(), reason))
}

/// Câu trả lời `/fixbugs` từ kết quả pull.
pub fn pull_reply(project_id: &str, report: &crate::fixbug::pull::PullReport) -> String {
    let mut lines = Vec::new();
    if report.new.is_empty() {
        lines.push(format!("{project_id}: không có bug mới."));
    } else {
        lines.push(format!(
            "{project_id}: đã lấy {} bug mới vào hàng chờ:",
            report.new.len()
        ));
        for b in report.new.iter().take(MAX_NEW_LINES) {
            lines.push(format!("{} · {} · {}", b.key, b.priority, b.title));
        }
        if report.new.len() > MAX_NEW_LINES {
            lines.push(format!("… và {} bug khác", report.new.len() - MAX_NEW_LINES));
        }
    }
    if report.skipped_existing > 0 {
        lines.push(format!("Bỏ qua {} bug đã có.", report.skipped_existing));
    }
    if !report.warnings.is_empty() {
        lines.push("Cảnh báo:".to_string());
        for w in report.warnings.iter().take(MAX_WARNING_LINES) {
            lines.push(format!("- {w}"));
        }
        if report.warnings.len() > MAX_WARNING_LINES {
            lines.push(format!(
                "… và {} cảnh báo khác",
                report.warnings.len() - MAX_WARNING_LINES
            ));
        }
    }
    clip(&lines.join("\n"), MAX_TEXT)
}

/// Câu trả lời `/bugs` từ tiến độ bug.
pub fn progress_reply(project_id: &str, rows: &[crate::db::FixbugProgress]) -> String {
    if rows.is_empty() {
        return format!("{project_id}: chưa có bug nào được lấy.");
    }
    let mut lines = vec![format!("Bug của {project_id} ({}):", rows.len())];
    for r in rows.iter().take(MAX_PROGRESS_LINES) {
        let task = match (r.task_id, r.task_status.as_deref()) {
            (Some(id), Some(st)) => format!("task #{id} {st}"),
            (Some(id), None) => format!("task #{id}"),
            _ if r.is_legacy() => "skill cũ".to_string(),
            _ => match (r.proposal_id, r.proposal_status.as_deref()) {
                (Some(id), Some(st)) => format!("đề xuất #{id} {st}"),
                _ => "chưa có task".to_string(),
            },
        };
        lines.push(format!("{} · {} · {task}", r.key, r.ref_state));
    }
    if rows.len() > MAX_PROGRESS_LINES {
        lines.push(format!("… và {} bug khác", rows.len() - MAX_PROGRESS_LINES));
    }
    clip(&lines.join("\n"), MAX_TEXT)
}

/// Dự án trong `telegram_projects` khớp `query`.
fn resolve_allowed(ctx: &BotCtx, query: &str) -> Result<Project, String> {
    let map = load_map(&ctx.st.map_path).map_err(|e| format!("Lỗi đọc projects_map.json: {}", e.message))?;
    let (projects, _) = allowed_projects(&map, &ctx.st.cfg)?;
    crate::routing::resolve(query, &projects)
        .map_err(|_| format!("Không tìm thấy dự án '{query}' trong telegram_projects."))
}

/// Xoá key, token và board id Trello của dự án khỏi câu trả lời.
fn scrub(project: &Project, text: &str) -> String {
    let root = std::path::Path::new(&project.path);
    let env = crate::fixbug::envfile::load_env(root);
    let mut secrets: Vec<String> = [env.trello_api_key(), env.trello_token(), env.trello_board_id()]
        .into_iter()
        .flatten()
        .map(str::to_string)
        .collect();
    if let Ok(cfg) = crate::fixbug::config::load_config(root) {
        for src in cfg.sources {
            if let crate::fixbug::config::SourceConfig::Trello(t) = src {
                if let Some(b) = t.board {
                    secrets.push(b);
                }
            }
        }
    }
    secrets.retain(|s| s.trim().len() >= 4);
    crate::secrets::redact_str(text, &secrets)
}

fn audit(text: &str) {
    crate::term::emit_out(&format!("Telegram: fixbug — {text}"));
}

/// Chạy `job` trên luồng riêng (một lệnh fixbug mỗi lúc), gửi kết quả về chat.
fn spawn_job<F>(ctx: &Arc<BotCtx>, job: F)
where
    F: FnOnce(&BotCtx) -> String + Send + 'static,
{
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        bot::reply(ctx, BUSY);
        return;
    }
    let ctx2 = Arc::clone(ctx);
    let spawned = std::thread::Builder::new()
        .name("telegram-fixbug".to_string())
        .spawn(move || {
            let text = job(&ctx2);
            RUNNING.store(false, Ordering::SeqCst);
            bot::reply(&ctx2, &text);
        });
    if spawned.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
        bot::reply(ctx, "Lỗi nội bộ — không chạy được lệnh fixbug.");
    }
}

/// Mở CSDL và tìm id dự án.
fn open_project(ctx: &BotCtx, project: &Project) -> Result<(rusqlite::Connection, i64), String> {
    let conn = crate::db::open(&ctx.st.db_path).map_err(|e| format!("Lỗi CSDL: {}", e.message))?;
    let id = crate::db::project_row_id(&conn, &project.id)
        .map_err(|_| format!("Dự án '{}' chưa có trong CSDL.", project.id))?;
    Ok((conn, id))
}

/// `/fixbugs <dự án> [số]`: `pull_guard` rồi `pull`.
pub fn on_fixbugs(ctx: &Arc<BotCtx>, rest: &str) {
    let (query, max) = match parse_fixbug_cmd(rest) {
        Ok(v) => v,
        Err(usage) => return bot::reply(ctx, &usage),
    };
    let project = match resolve_allowed(ctx, &query) {
        Ok(p) => p,
        Err(msg) => return bot::reply(ctx, &msg),
    };
    bot::reply(ctx, &format!("Đang lấy bug của {}…", project.id));
    spawn_job(ctx, move |ctx| {
        let text = match open_project(ctx, &project) {
            Err(msg) => msg,
            Ok((mut conn, project_id)) => {
                let root = std::path::Path::new(&project.path);
                let res = crate::fixbug::pull_guard(root, &conn, project_id).and_then(|_| {
                    crate::fixbug::pull::pull(&mut conn, root, project_id, &project.name_vi, Some(max))
                });
                match res {
                    Ok(report) => {
                        audit(&format!("/fixbugs {} ({} bug mới)", project.id, report.new.len()));
                        pull_reply(&project.id, &report)
                    }
                    Err(e) => {
                        audit(&format!("/fixbugs {} lỗi", project.id));
                        format!("Không lấy được bug: {e}")
                    }
                }
            }
        };
        scrub(&project, &text)
    });
}

/// `/bugs <dự án>`: tiến độ từng bug (chỉ đọc CSDL, chạy ngay).
pub fn on_bugs(ctx: &Arc<BotCtx>, rest: &str) {
    let query = match parse_bugs_args(rest) {
        Ok(v) => v,
        Err(usage) => return bot::reply(ctx, &usage),
    };
    let project = match resolve_allowed(ctx, &query) {
        Ok(p) => p,
        Err(msg) => return bot::reply(ctx, &msg),
    };
    let text = match open_project(ctx, &project) {
        Err(msg) => msg,
        Ok((conn, project_id)) => match crate::db::fixbug_progress(&conn, project_id) {
            Ok(rows) => progress_reply(&project.id, &rows),
            Err(e) => format!("Lỗi CSDL: {}", e.message),
        },
    };
    bot::reply(ctx, &text);
}

fn writeback_reply(
    res: Result<Vec<crate::fixbug::writeback::WritebackItemResult>, crate::fixbug::FixbugError>,
    what: &str,
) -> String {
    match res {
        Ok(items) if items.is_empty() => "Không có bug nào được cập nhật.".to_string(),
        Ok(items) => {
            let lines: Vec<String> = items
                .iter()
                .map(|i| format!("✓ {}: {}", i.key, i.message))
                .collect();
            clip(&lines.join("\n"), MAX_TEXT)
        }
        Err(e) => format!("Lỗi {what}: {e}"),
    }
}

/// `/deployed <dự án> <KEY...> [-- ghi chú]`: cùng hàm với `fixbug deployed`.
pub fn on_deployed(ctx: &Arc<BotCtx>, rest: &str) {
    let (query, keys, note) = match parse_deployed_args(rest) {
        Ok(v) => v,
        Err(usage) => return bot::reply(ctx, &usage),
    };
    let project = match resolve_allowed(ctx, &query) {
        Ok(p) => p,
        Err(msg) => return bot::reply(ctx, &msg),
    };
    spawn_job(ctx, move |ctx| {
        let text = match open_project(ctx, &project) {
            Err(msg) => msg,
            Ok((mut conn, project_id)) => {
                let root = std::path::Path::new(&project.path);
                let res = crate::fixbug::writeback::deployed(
                    &mut conn,
                    root,
                    project_id,
                    &keys,
                    note.as_deref(),
                );
                audit(&format!(
                    "/deployed {} {} ({})",
                    project.id,
                    keys.join(" "),
                    if res.is_ok() { "xong" } else { "lỗi" }
                ));
                writeback_reply(res, "deployed")
            }
        };
        scrub(&project, &text)
    });
}

/// `/reject <dự án> <KEY> <lý do>`: cùng hàm với `fixbug reject`.
pub fn on_reject(ctx: &Arc<BotCtx>, rest: &str) {
    let (query, key, reason) = match parse_reject_args(rest) {
        Ok(v) => v,
        Err(usage) => return bot::reply(ctx, &usage),
    };
    let project = match resolve_allowed(ctx, &query) {
        Ok(p) => p,
        Err(msg) => return bot::reply(ctx, &msg),
    };
    spawn_job(ctx, move |ctx| {
        let text = match open_project(ctx, &project) {
            Err(msg) => msg,
            Ok((mut conn, project_id)) => {
                let root = std::path::Path::new(&project.path);
                let keys = vec![key.clone()];
                let res =
                    crate::fixbug::writeback::reject(&mut conn, root, project_id, &keys, &reason);
                audit(&format!(
                    "/reject {} {key} ({})",
                    project.id,
                    if res.is_ok() { "xong" } else { "lỗi" }
                ));
                writeback_reply(res, "reject")
            }
        };
        scrub(&project, &text)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixbug_cmd_rules() {
        assert_eq!(parse_fixbug_cmd("bizsurvey"), Ok(("bizsurvey".to_string(), DEFAULT_MAX_NEW)));
        assert_eq!(parse_fixbug_cmd(" bizsurvey 3 "), Ok(("bizsurvey".to_string(), 3)));
        assert_eq!(parse_fixbug_cmd("bizsurvey 99"), Ok(("bizsurvey".to_string(), 20)));
        assert!(parse_fixbug_cmd("").is_err());
        assert!(parse_fixbug_cmd("bizsurvey 0").is_err());
        assert!(parse_fixbug_cmd("bizsurvey abc").is_err());
        assert!(parse_fixbug_cmd("a 1 2").is_err());
    }

    #[test]
    fn parse_bugs_rules() {
        assert_eq!(parse_bugs_args(" apec "), Ok("apec".to_string()));
        assert!(parse_bugs_args("").is_err());
        assert!(parse_bugs_args("a b").is_err());
    }

    #[test]
    fn parse_deployed_rules() {
        assert_eq!(
            parse_deployed_args("biz T-1 T-2"),
            Ok(("biz".to_string(), vec!["T-1".to_string(), "T-2".to_string()], None))
        );
        assert_eq!(
            parse_deployed_args("biz T-1 -- đã lên  staging"),
            Ok((
                "biz".to_string(),
                vec!["T-1".to_string()],
                Some("đã lên staging".to_string())
            ))
        );
        assert_eq!(
            parse_deployed_args("biz T-1 --"),
            Ok(("biz".to_string(), vec!["T-1".to_string()], None))
        );
        // `--` inside a word is not the separator.
        assert_eq!(
            parse_deployed_args("biz T-1 -- bản a--b"),
            Ok(("biz".to_string(), vec!["T-1".to_string()], Some("bản a--b".to_string())))
        );
        assert!(parse_deployed_args("biz").is_err());
        assert!(parse_deployed_args("biz -- note").is_err());
        assert!(parse_deployed_args("").is_err());
    }

    #[test]
    fn parse_reject_rules() {
        assert_eq!(
            parse_reject_args("biz T-3 không tái hiện được"),
            Ok(("biz".to_string(), "T-3".to_string(), "không tái hiện được".to_string()))
        );
        assert_eq!(parse_reject_args("biz T-3"), Err(USAGE_REJECT.to_string()));
        assert!(parse_reject_args("biz").is_err());
    }

    #[test]
    fn pull_reply_caps_lines() {
        let report = crate::fixbug::pull::PullReport {
            new: (1..=25)
                .map(|i| crate::fixbug::pull::NewBugInfo {
                    key: format!("T-{i}"),
                    title: format!("Bug {i}"),
                    priority: "normal".to_string(),
                })
                .collect(),
            skipped_existing: 2,
            warnings: (1..=7).map(|i| format!("w{i}")).collect(),
        };
        let text = pull_reply("biz", &report);
        assert!(text.contains("đã lấy 25 bug mới"), "{text}");
        assert!(text.contains("T-20 · normal · Bug 20"), "{text}");
        assert!(!text.contains("T-21 ·"), "{text}");
        assert!(text.contains("… và 5 bug khác"), "{text}");
        assert!(text.contains("Bỏ qua 2 bug đã có."), "{text}");
        assert!(text.contains("- w5"), "{text}");
        assert!(!text.contains("- w6"), "{text}");
        assert!(text.contains("… và 2 cảnh báo khác"), "{text}");
    }
}
