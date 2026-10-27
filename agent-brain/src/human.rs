//! Bước Dev do nhân sự làm (engine `người`, D-19..D-25): agent-brain chuẩn bị và đẩy nhánh task,
//! chờ mà không có tiến trình hay run `running` nào, lấy nhánh nhân sự đã đẩy bằng fast-forward
//! khi người dùng bấm Xong, kiểm tra diff như diff của agent rồi ghi lại việc đó thành một run
//! đã xong. Lịch sử cục bộ không bao giờ bị viết lại: không ép đẩy, không kéo, không đặt lại.

use crate::app::Ctx;
use crate::db::TaskRow;
use crate::error::{BrainError, BrainResult, EXIT_OK, EXIT_PROTOCOL, EXIT_WAITING};
use crate::projects::Project;
use crate::teams::{HumanWait, PushResult, TaskTeam};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Chờ nhân sự làm (hoặc làm lại sau khi được trả).
pub const MODE_WAITING: &str = "waiting";
/// Kết quả lần trước chưa đạt; chờ người dùng bấm Trả lại.
pub const MODE_REJECTED: &str = "rejected";

/// Giá trị `PushResult.error` khi dự án chưa có remote `origin`.
const NO_ORIGIN: &str = "chưa có remote origin";

/// `^task/[a-z0-9][a-z0-9-]*$`, tổng độ dài tối đa 45.
pub fn valid_task_branch(b: &str) -> bool {
    let Some(rest) = b.strip_prefix("task/") else {
        return false;
    };
    if b.len() > 45 {
        return false;
    }
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Lệnh người dùng chạy tay khi agent-brain không đẩy được nhánh.
pub fn push_command(branch: &str) -> String {
    format!("git push -u origin {branch}")
}

/// Đẩy nhánh task lên `origin` (D-25): chỉ khi có `origin`, không bao giờ ép đẩy. Kết quả được
/// lưu vào marker; thất bại không dừng việc chờ.
pub fn try_push(root: &Path, branch: &str) -> PushResult {
    let command = push_command(branch);
    if !valid_task_branch(branch) {
        return PushResult {
            ok: false,
            command,
            error: Some("tên nhánh không hợp lệ".to_string()),
        };
    }
    if crate::gitops::git(root, &["remote", "get-url", "origin"]).is_err() {
        return PushResult {
            ok: false,
            command,
            error: Some(NO_ORIGIN.to_string()),
        };
    }
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    match crate::gitops::git(root, &["push", "-u", "origin", &refspec]) {
        Ok(_) => PushResult {
            ok: true,
            command,
            error: None,
        },
        Err(e) => PushResult {
            ok: false,
            command,
            error: Some(e.message.lines().next().unwrap_or("").trim().to_string()),
        },
    }
}

fn print_push(branch: &str, push: &PushResult) {
    if push.ok {
        crate::term::emit_out(&format!("Đã đẩy nhánh {branch} lên remote origin."));
    } else if push.error.as_deref() == Some(NO_ORIGIN) {
        crate::term::emit_out(&format!(
            "Dự án chưa có remote origin — đẩy nhánh bằng tay: {}",
            push.command
        ));
    } else {
        crate::term::emit_out(&format!(
            "Không đẩy được nhánh lên remote ({}) — chạy tay: {}",
            push.error.as_deref().unwrap_or("không rõ lỗi"),
            push.command
        ));
    }
}

fn print_status(task_id: i64, name: &str, wait: &HumanWait) {
    if wait.mode == MODE_REJECTED {
        crate::term::emit_out(&format!(
            "Kết quả của {name} chưa đạt — gửi báo cáo {} cho {name} rồi bấm Trả lại (agent-brain human return {task_id}).",
            wait.report_paths.join(", ")
        ));
        return;
    }
    let step = if wait.stage == "fix" { "Sửa" } else { "Code" };
    crate::term::emit_out(&format!(
        "Chờ {name} làm bước {step} trên nhánh {}.",
        wait.branch
    ));
}

fn print_done_hint(task_id: i64, name: &str) {
    crate::term::emit_out(&format!(
        "Khi {name} báo xong: bấm Xong trên dashboard hoặc chạy agent-brain human done {task_id}."
    ));
}

fn report_file_for(fix_for: &str) -> Option<&'static str> {
    match fix_for {
        "qa" => Some(crate::report::QA_REPORT_FILE),
        "code_review" => Some(crate::report::CODE_REVIEW_FILE),
        _ => None,
    }
}

fn autopilot_flag() -> bool {
    std::env::var(crate::autopilot::AUTOPILOT_TASK_ENV)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn team_changed() -> BrainError {
    BrainError::error("Đội hình của task vừa đổi — chạy lại.")
}

/// Bắt đầu một bước Dev do người làm: ghi marker `wait` vào `team_json`, đẩy nhánh (cố gắng hết
/// sức) và trả `EXIT_WAITING`. Không chạy tiến trình, không thêm run nào.
#[allow(clippy::too_many_arguments)]
pub fn start_wait(
    ctx: &mut Ctx,
    _project: &Project,
    root: &Path,
    task: &TaskRow,
    staff_id: &str,
    stage: &str,
    fix_for: Option<&str>,
    round: Option<i64>,
) -> BrainResult<i32> {
    let staff = crate::staff::load();
    if staff.get(staff_id).is_none() {
        return Err(BrainError::error(format!(
            "Nhân sự {staff_id} không còn trong danh sách — đổi đội hình rồi chạy lại."
        )));
    }
    let name = staff.display_name_of(staff_id);

    if !valid_task_branch(&task.branch) {
        return Err(BrainError::error(format!(
            "Nhánh '{}' không phải nhánh task hợp lệ — không giao cho nhân sự.",
            task.branch
        )));
    }

    let raw = crate::db::task_team_json(&ctx.conn, task.id)?;
    let mut team = TaskTeam::parse(raw.as_deref()).ok_or_else(|| {
        BrainError::error(format!(
            "Không đọc được đội hình của task {} — đổi đội hình rồi chạy lại.",
            task.id
        ))
    })?;

    if let Some(existing) = &team.wait {
        // Marker đã có: chỉ in trạng thái, không ghi gì.
        let ename = staff.display_name_of(&existing.staff);
        print_status(task.id, &ename, existing);
        if let Some(push) = &existing.push {
            print_push(&existing.branch, push);
        }
        if existing.mode == MODE_WAITING {
            print_done_hint(task.id, &ename);
        }
        return Ok(EXIT_WAITING);
    }

    let report_paths: Vec<String> = fix_for
        .and_then(report_file_for)
        .map(|f| vec![f.to_string()])
        .unwrap_or_default();
    let mode = if fix_for.is_some() {
        MODE_REJECTED
    } else {
        MODE_WAITING
    };

    let base = crate::gitops::head_sha(root)?;
    let push = try_push(root, &task.branch);
    let wait = HumanWait {
        mode: mode.to_string(),
        role: "dev".to_string(),
        stage: stage.to_string(),
        fix_for: fix_for.map(str::to_string),
        round,
        staff: staff_id.to_string(),
        since_ms: crate::db::now_ms(),
        branch: task.branch.clone(),
        base,
        report_paths,
        push: Some(push.clone()),
        autopilot: autopilot_flag(),
    };
    team.wait = Some(wait.clone());
    if !crate::db::set_task_team_cas(&ctx.conn, task.id, raw.as_deref(), &team.to_json())? {
        return Err(team_changed());
    }

    print_status(task.id, &name, &wait);
    print_push(&task.branch, &push);
    if wait.mode == MODE_WAITING {
        print_done_hint(task.id, &name);
    }
    Ok(EXIT_WAITING)
}

/// Mọi thứ `done` và `give_back` cần về một task đang chờ nhân sự.
struct Pending {
    task: TaskRow,
    team: TaskTeam,
    raw: Option<String>,
    wait: HumanWait,
    profile: String,
    root: PathBuf,
    name: String,
}

fn load_pending(ctx: &Ctx, task_id: i64) -> BrainResult<Pending> {
    let task = crate::db::get_task(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy task {task_id}")))?;
    let raw = crate::db::task_team_json(&ctx.conn, task_id)?;
    let not_waiting = || BrainError::error(format!("Task {task_id} không chờ nhân sự làm"));
    let team = TaskTeam::parse(raw.as_deref()).ok_or_else(not_waiting)?;
    let wait = team.wait.clone().ok_or_else(not_waiting)?;

    let (slug, profile) = crate::autopilot::claim::task_project(&ctx.conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy dự án của task {task_id}")))?;
    let project = ctx
        .projects
        .iter()
        .find(|p| p.id == slug)
        .cloned()
        .ok_or_else(|| {
            BrainError::error(format!("Dự án '{slug}' không còn trong projects_map.json"))
        })?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
    let name = crate::staff::load().display_name_of(&wait.staff);
    Ok(Pending {
        task,
        team,
        raw,
        wait,
        profile,
        root,
        name,
    })
}

/// Các file cho phép đổi: file trong bảng Thay đổi của plan lúc bắt đầu bước (NFC) và `.plan.md`.
fn allowed_paths(root: &Path, base: &str) -> BTreeSet<String> {
    let plan_text = crate::gitops::git(root, &["show", &format!("{base}:.plan.md")])
        .ok()
        .or_else(|| std::fs::read_to_string(root.join(".plan.md")).ok())
        .unwrap_or_default();
    let plan = crate::planfile::parse_plan(&plan_text);
    let mut allowed: BTreeSet<String> = crate::planfile::plan_files(&plan)
        .iter()
        .map(|f| crate::planfile::normalize_plan_path(f).nfc().collect())
        .collect();
    allowed.insert(".plan.md".to_string());
    allowed
}

/// Người dùng bấm Xong: lấy nhánh nhân sự đã đẩy (fetch + fast-forward), kiểm tra diff, ghi run.
pub fn done(ctx: &mut Ctx, task_id: i64) -> BrainResult<i32> {
    let p = load_pending(ctx, task_id)?;
    let name = p.name.clone();
    let branch = p.wait.branch.clone();

    if p.wait.mode == MODE_REJECTED {
        return Err(BrainError::error(format!(
            "Bấm Trả lại trước: kết quả lần trước của {name} chưa đạt."
        )));
    }
    if !valid_task_branch(&branch) || branch != p.task.branch {
        return Err(BrainError::error(format!(
            "Nhánh '{branch}' không phải nhánh task hợp lệ của task {task_id} — không lấy."
        )));
    }

    let root = p.root.as_path();
    let cur = crate::gitops::current_branch(root);
    if cur.as_deref() != Some(branch.as_str()) {
        return Err(BrainError::error(format!(
            "Không ở nhánh task '{branch}' (nhánh hiện tại: '{}') — không đoán",
            cur.as_deref().unwrap_or("None")
        )));
    }

    let dirty: Vec<String> = crate::gitops::changed_paths(root)?
        .into_iter()
        .filter(|f| !crate::stream::is_protocol_file(f))
        .collect();
    if !dirty.is_empty() {
        let list = dirty
            .iter()
            .map(|f| format!("  {f}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BrainError::error(format!(
            "Working tree có thay đổi chưa commit — không lấy được nhánh của {name}:\n{list}"
        )));
    }

    if crate::gitops::git(root, &["remote", "get-url", "origin"]).is_err() {
        return Err(BrainError::error(format!(
            "Dự án chưa có remote origin — không lấy được nhánh của {name}."
        )));
    }

    let remote_ref = format!("refs/heads/{branch}");
    if let Err(e) = crate::gitops::git(root, &["fetch", "origin", &remote_ref]) {
        // Phân biệt nhánh chưa có trên remote với lỗi mạng / quyền, không dựa vào lời nhắn của git.
        let missing = matches!(
            crate::gitops::git(root, &["ls-remote", "origin", &remote_ref]),
            Ok(out) if out.trim().is_empty()
        );
        if missing {
            return Err(BrainError::error(format!(
                "Chưa thấy nhánh {branch} trên remote — nhờ {name} đẩy nhánh lên rồi bấm Xong lại."
            )));
        }
        let first = e.message.lines().next().unwrap_or("").trim().to_string();
        return Err(BrainError::error(format!(
            "Không lấy được nhánh {branch} từ remote ({first}) — thử lại sau."
        )));
    }

    let remote_sha = crate::gitops::git(root, &["rev-parse", "FETCH_HEAD"])?
        .trim()
        .to_string();
    let head = crate::gitops::head_sha(root)?;
    if remote_sha == head {
        return Err(BrainError::error(format!(
            "Nhánh {branch} trên remote chưa có commit mới nào — nhờ {name} đẩy việc đã làm rồi bấm Xong lại."
        )));
    }
    if !crate::gitops::is_ancestor(root, &head, &remote_sha) {
        return Err(BrainError::error(format!(
            "Nhánh {branch} trên remote không đi tiếp từ nhánh trong máy (có thể đã bị viết lại lịch sử) — không gộp. Kiểm tra với {name} rồi thử lại."
        )));
    }
    crate::gitops::git(root, &["merge", "--ff-only", "FETCH_HEAD"])?;
    let new_head = crate::gitops::head_sha(root)?;

    // D-20: cùng luật với diff của một Dev agent; không tự hoàn tác gì.
    let allowed = allowed_paths(root, &p.wait.base);
    let changed: BTreeSet<String> = crate::gitops::diff_names(root, &p.wait.base, &new_head)?
        .into_iter()
        .map(|f| f.nfc().collect())
        .collect();
    let mut violations: Vec<String> = changed
        .iter()
        .filter(|f| {
            !allowed.contains(*f) || f.starts_with(".planning/") || crate::report::sensitive_path(f)
        })
        .cloned()
        .collect();
    violations.sort();
    let violated = !violations.is_empty();

    let now = crate::db::now_ms();
    let mut metrics = serde_json::json!({
        "team": p.team.name,
        "staff": p.wait.staff,
        "human": {
            "waited_ms": (now - p.wait.since_ms).max(0),
            "branch": branch,
            "base": p.wait.base,
            "head": new_head,
        },
    });
    if let Some(f) = &p.wait.fix_for {
        metrics["fix_for"] = serde_json::Value::String(f.clone());
    }
    let run = crate::db::HumanRun {
        task_id,
        project_id: p.task.project_id,
        role: "dev".to_string(),
        profile: p.profile.clone(),
        stage: Some(p.wait.stage.clone()),
        round: p.wait.round,
        started_at: p.wait.since_ms,
        ended_at: now,
        outcome: violated.then(|| "protocol_violation".to_string()),
        base_commit: Some(p.wait.base.clone()),
        metrics_json: metrics.to_string(),
    };

    let mut cleared = p.team.clone();
    cleared.wait = None;
    let run_id = {
        let tx = ctx.conn.transaction()?;
        let run_id = crate::db::insert_human_run(&tx, &run)?;
        if !crate::db::set_task_team_cas(&tx, task_id, p.raw.as_deref(), &cleared.to_json())? {
            return Err(team_changed());
        }
        if !violated {
            crate::db::set_task_status(&tx, task_id, "dev_done")?;
        }
        tx.commit()?;
        run_id
    };

    let code = if violated {
        crate::term::emit_out(&format!(
            "{name} đã sửa file ngoài plan — vi phạm giao thức (ROLE-06):"
        ));
        for f in &violations {
            crate::term::emit_out(&format!("  {f}"));
        }
        crate::term::emit_out("agent-brain không tự hoàn tác");
        EXIT_PROTOCOL
    } else {
        crate::term::emit_out(&format!(
            "Đã lấy nhánh {branch} của {name} — run {run_id} (người)."
        ));
        EXIT_OK
    };

    // Autopilot chạy tiếp ngay trong tiến trình này, kể cả sau vi phạm để `decide` ghi lý do dừng.
    if p.wait.autopilot && violated {
        return crate::autopilot::stop_after_human_violation(ctx, task_id);
    }
    if p.wait.autopilot {
        return crate::autopilot::cmd_auto(
            ctx,
            &crate::autopilot::AutoOpts {
                args: vec![task_id.to_string()],
                ..Default::default()
            },
        );
    }
    Ok(code)
}

/// Người dùng bấm Trả lại: kết quả chưa đạt, đẩy lại nhánh và chờ nhân sự làm lại.
pub fn give_back(ctx: &mut Ctx, task_id: i64) -> BrainResult<i32> {
    let p = load_pending(ctx, task_id)?;
    if p.wait.mode != MODE_REJECTED {
        return Err(BrainError::error(format!(
            "Chỉ trả lại khi kết quả của {} chưa đạt.",
            p.name
        )));
    }
    let push = try_push(&p.root, &p.wait.branch);
    let mut wait = p.wait.clone();
    wait.mode = MODE_WAITING.to_string();
    wait.since_ms = crate::db::now_ms();
    wait.push = Some(push.clone());
    let mut team = p.team.clone();
    team.wait = Some(wait);
    if !crate::db::set_task_team_cas(&ctx.conn, task_id, p.raw.as_deref(), &team.to_json())? {
        return Err(team_changed());
    }
    print_push(&p.wait.branch, &push);
    crate::term::emit_out(&format!("Đã trả việc lại cho {}.", p.name));
    Ok(EXIT_OK)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanAction {
    Done,
    Return,
}

/// `agent-brain human done|return <task_id>`.
pub fn cmd_human(ctx: &mut Ctx, action: HumanAction, task_id: i64) -> BrainResult<i32> {
    match action {
        HumanAction::Done => done(ctx, task_id),
        HumanAction::Return => give_back(ctx, task_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_validation() {
        assert!(valid_task_branch("task/sua-app"));
        assert!(valid_task_branch("task/a"));
        assert!(valid_task_branch("task/1-2-3"));
        assert!(!valid_task_branch("task/"));
        assert!(!valid_task_branch("task/-x"));
        assert!(!valid_task_branch("task/Sua"));
        assert!(!valid_task_branch("task/a b"));
        assert!(!valid_task_branch("task/a/b"));
        assert!(!valid_task_branch("main"));
        assert!(!valid_task_branch("task/sửa"));
        let longest = format!("task/{}", "a".repeat(40));
        assert_eq!(longest.len(), 45);
        assert!(valid_task_branch(&longest));
        assert!(!valid_task_branch(&format!("{longest}a")));
    }

    #[test]
    fn push_command_text() {
        assert_eq!(
            push_command("task/sua-app"),
            "git push -u origin task/sua-app"
        );
    }
}
