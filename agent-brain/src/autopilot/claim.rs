//! Nhận task cho driver autopilot (một driver cho một task) và trả task khi dừng.
//!
//! Mọi kiểm tra "đang bận" dựa trên cặp (pid, thời điểm khởi động) qua
//! `procs::alive`, nên một driver đã chết để lại hàng `running` vẫn nhận lại được.

use crate::autopilot::decide::{AutoJson, Replan, StopReason};
use crate::config::Price;
use crate::db::{DriverRow, RunRow, TaskAuto, TaskRow};
use crate::error::{BrainError, BrainResult};
use crate::procs::ProcId;
use rusqlite::{Connection, OptionalExtension};
use std::collections::BTreeMap;
use std::path::Path;

fn proc_of(pid: Option<i64>, start: Option<i64>) -> Option<ProcId> {
    match (pid, start) {
        (Some(p), Some(s)) if p > 0 && s >= 0 => Some(ProcId {
            pid: p as i32,
            start: s as u64,
        }),
        _ => None,
    }
}

/// Driver của task còn sống: auto_state `running` và tiến trình (pid, start) còn chạy.
pub fn driver_alive(a: &TaskAuto) -> bool {
    if a.auto_state.as_deref() != Some("running") {
        return false;
    }
    match proc_of(a.auto_pid, a.auto_pid_start) {
        Some(p) => crate::procs::alive(p),
        None => false,
    }
}

/// Hàng driver (auto_state `running`) còn sống.
pub fn row_alive(r: &DriverRow) -> bool {
    match proc_of(r.auto_pid, r.auto_pid_start) {
        Some(p) => crate::procs::alive(p),
        None => false,
    }
}

fn run_alive(r: &RunRow) -> bool {
    match proc_of(r.pid, r.pid_start) {
        Some(p) => crate::procs::alive(p),
        None => false,
    }
}

/// Lý do một dự án / profile đang bị một driver autopilot khác chiếm (None = rảnh).
pub fn live_driver_reason(
    conn: &Connection,
    project_slug: &str,
    profile: &str,
    exclude_task: Option<i64>,
) -> BrainResult<Option<String>> {
    for r in crate::db::auto_driver_rows(conn)? {
        if Some(r.task_id) == exclude_task {
            continue;
        }
        if !row_alive(&r) {
            continue;
        }
        if !project_slug.is_empty() && r.project_slug == project_slug {
            return Ok(Some(format!(
                "Dự án {project_slug} đang chạy autopilot cho task {} — chờ autopilot dừng hoặc bấm Dừng autopilot",
                r.task_id
            )));
        }
        if profile == "personal" && r.profile == "personal" {
            return Ok(Some(format!(
                "Profile personal đang chạy autopilot cho task {} — không chạy song song trên cùng thư mục cấu hình (E-06)",
                r.task_id
            )));
        }
    }
    Ok(None)
}

/// Tasks of a project waiting at the plan gate (D2.3a): `auto_state = 'gate'` and the plan not
/// approved since the latest Lead run started (an approval recorded without running on leaves
/// `gate` in place, and must not hold the project).
pub fn gate_waiting_tasks(conn: &Connection, project_slug: &str) -> BrainResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT tasks.id FROM tasks JOIN projects ON tasks.project_id = projects.id
         WHERE projects.slug = ?1 AND tasks.auto_state = 'gate'
           AND (tasks.approved_at IS NULL OR tasks.approved_at < COALESCE(
                (SELECT MAX(runs.started_at) FROM runs WHERE runs.task_id = tasks.id AND runs.role = 'lead'), 0))
         ORDER BY tasks.id",
    )?;
    let ids = stmt
        .query_map(rusqlite::params![project_slug], |r| r.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// D2.3a (`hold_project_at_gate`): another task of the project waits at the plan gate, so the
/// project stays with it. `exclude_task` is the task asking: its own gate never holds it, and a
/// task that itself waits at the gate is not held by another gated task (whichever is approved
/// first runs; the other then waits for its run).
pub fn gate_hold_reason(
    conn: &Connection,
    project_slug: &str,
    exclude_task: Option<i64>,
) -> BrainResult<Option<String>> {
    let waiting = gate_waiting_tasks(conn, project_slug)?;
    if let Some(own) = exclude_task {
        if waiting.contains(&own) {
            return Ok(None);
        }
    }
    Ok(waiting.into_iter().find(|t| Some(*t) != exclude_task).map(|t| {
        format!(
            "Dự án {project_slug} đang giữ cho task {t} chờ duyệt plan ({}) — duyệt plan hoặc dừng autopilot task {t} trước",
            crate::projects::HOLD_AT_GATE_KEY
        )
    }))
}

/// Dự án (hoặc profile personal) đang có run của task khác hoặc driver khác; với
/// `hold_at_gate` (`hold_project_at_gate` của dự án) thêm task khác đang chờ duyệt plan.
pub fn project_busy_reason(
    conn: &Connection,
    project_slug: &str,
    profile: &str,
    exclude_task: Option<i64>,
    hold_at_gate: bool,
) -> BrainResult<Option<String>> {
    let running = crate::db::running_runs(conn)?;
    for r in &running {
        if r.task_id.is_some() && r.task_id == exclude_task {
            continue;
        }
        if !run_alive(r) {
            continue;
        }
        let slug: Option<String> = conn
            .query_row(
                "SELECT slug FROM projects WHERE id = ?1",
                rusqlite::params![r.project_id],
                |row| row.get(0),
            )
            .ok();
        if slug.as_deref() == Some(project_slug) {
            let tid = r
                .task_id
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string());
            return Ok(Some(format!(
                "Dự án {project_slug} đang có run {} ({}, task {tid}) chạy — chờ run xong hoặc bấm Kill",
                r.id, r.role
            )));
        }
    }
    if profile == "personal" {
        for r in &running {
            if r.task_id.is_some() && r.task_id == exclude_task {
                continue;
            }
            if r.profile != "personal" || !run_alive(r) {
                continue;
            }
            let tid = r
                .task_id
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string());
            return Ok(Some(format!(
                "Profile personal đang có run {} chạy (task {tid}) — không chạy song song trên cùng thư mục cấu hình (E-06)",
                r.id
            )));
        }
    }
    if let Some(reason) = live_driver_reason(conn, project_slug, profile, exclude_task)? {
        return Ok(Some(reason));
    }
    if hold_at_gate {
        return gate_hold_reason(conn, project_slug, exclude_task);
    }
    Ok(None)
}

/// What approving does about the branch (D2.3b, `auto_checkout_on_approve`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApproveBranch {
    /// The repo is on the task branch already.
    OnBranch,
    /// Not on the task branch, and agent-brain checks it out before approving.
    Checkout,
    /// Not on the task branch and no checkout: the reason (the dirty files when that is why).
    Refuse(String),
}

/// Decides `ApproveBranch` from the repo facts. Checkout only when the option is on, the task
/// branch exists, the working tree is clean and no other task's run / driver holds the project
/// (`busy`).
pub fn approve_branch(
    auto_checkout: bool,
    branch: &str,
    current: Option<&str>,
    branch_exists: bool,
    dirty: &[String],
    busy: Option<&str>,
) -> ApproveBranch {
    if current == Some(branch) {
        return ApproveBranch::OnBranch;
    }
    let mismatch = format!(
        "Không ở nhánh task '{branch}' (nhánh hiện tại: '{}') — không đoán",
        current.unwrap_or("None")
    );
    if !auto_checkout || !branch_exists {
        return ApproveBranch::Refuse(mismatch);
    }
    if let Some(b) = busy {
        return ApproveBranch::Refuse(format!("{mismatch}. {b}"));
    }
    if !dirty.is_empty() {
        let mut list = dirty.to_vec();
        list.sort();
        return ApproveBranch::Refuse(format!(
            "{mismatch}. Không tự checkout vì working tree có thay đổi chưa commit:\n{}",
            list.join("\n")
        ));
    }
    ApproveBranch::Checkout
}

/// Reads the repo at `root` live and, when `approve_branch` says so, checks out the task branch.
/// `Ok(true)` = on the task branch now (already or after the checkout); `Ok(false)` = left as
/// is, with `Err(reason)` only when `required` (approving would run on and needs the branch).
pub fn checkout_for_approve(
    root: &Path,
    branch: &str,
    auto_checkout: bool,
    busy: Option<&str>,
    required: bool,
) -> Result<bool, String> {
    let current = crate::gitops::current_branch(root);
    let dirty: Vec<String> = crate::gitops::changed_paths(root)
        .map_err(|e| e.message)?
        .into_iter()
        .collect();
    let exists = crate::gitops::branch_exists(root, branch);
    match approve_branch(auto_checkout, branch, current.as_deref(), exists, &dirty, busy) {
        ApproveBranch::OnBranch => Ok(true),
        ApproveBranch::Checkout => match crate::gitops::checkout_branch(root, branch) {
            Ok(()) => {
                crate::term::emit(&format!(
                    "Đã checkout nhánh task '{branch}' (từ '{}') để duyệt plan",
                    current.as_deref().unwrap_or("None")
                ));
                Ok(true)
            }
            Err(e) if required => Err(format!("Không checkout được nhánh '{branch}': {}", e.message)),
            Err(_) => Ok(false),
        },
        ApproveBranch::Refuse(reason) if required => Err(reason),
        ApproveBranch::Refuse(_) => Ok(false),
    }
}

/// Tùy chọn khi nhận task: `Chạy thêm 1 vòng` và yêu cầu lập lại plan.
#[derive(Debug, Clone, Default)]
pub struct ClaimOpts {
    pub more_round: bool,
    pub replan: Option<Replan>,
}

/// Số vòng cấp thêm cho `Chạy thêm 1 vòng`: đủ để trần vượt vòng đã dừng
/// (G-01: Trả về Lead có thể chạy quá trần), tối thiểu 1.
fn more_round_grant(round: Option<i64>, cap: Option<i64>) -> i64 {
    match (round, cap) {
        (Some(r), Some(c)) => (r + 1 - c).max(1),
        _ => 1,
    }
}

/// Nhận task cho driver này trong một giao dịch BEGIN IMMEDIATE.
pub fn claim(
    conn: &mut Connection,
    task_id: i64,
    project_slug: &str,
    profile: &str,
    opts: &ClaimOpts,
    hold_at_gate: bool,
) -> BrainResult<AutoJson> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    if crate::db::get_task(&tx, task_id)?.is_none() {
        return Err(BrainError::error(format!("Không tìm thấy task {task_id}")));
    }
    let auto = crate::db::task_auto(&tx, task_id)?.unwrap_or_default();
    if auto.workflow_json.is_none() {
        return Err(BrainError::error(format!(
            "Task {task_id} tạo trước Phase 5.2 — không có quy trình để chạy autopilot."
        )));
    }
    if driver_alive(&auto) {
        return Err(BrainError::error(format!(
            "Autopilot đang chạy task {task_id} (pid {}) — không chạy hai autopilot cho một task",
            auto.auto_pid.unwrap_or(0)
        )));
    }
    if let Some(r) = crate::db::running_run_for_task(&tx, task_id)? {
        if run_alive(&r) {
            return Err(BrainError::error(format!(
                "Task {task_id} đang có run {} chạy — chờ run xong",
                r.id
            )));
        }
    }
    if let Some(reason) =
        project_busy_reason(&tx, project_slug, profile, Some(task_id), hold_at_gate)?
    {
        return Err(BrainError::error(reason));
    }

    let mut aj = AutoJson::parse(auto.auto_json.as_deref());
    if opts.more_round {
        if auto.auto_state.as_deref() != Some("stopped") {
            return Err(BrainError::error(
                "Chỉ chạy thêm 1 vòng khi autopilot đã dừng",
            ));
        }
        if let Some(stop) = aj.stop.as_ref() {
            match stop.reason {
                StopReason::HetVong => {
                    let key = stop
                        .loop_name
                        .map(|l| l.as_str())
                        .unwrap_or("plan")
                        .to_string();
                    *aj.grants.entry(key).or_insert(0) += more_round_grant(stop.round, stop.cap);
                }
                StopReason::HetNganSach => {
                    aj.bypass_stages = 2;
                }
                _ => {}
            }
        }
    }
    if let Some(rp) = opts.replan.as_ref() {
        aj.replan = Some(rp.clone());
    }
    aj.stop = None;
    aj.last_error = None;

    let own_pid = std::process::id() as i32;
    let own_start = crate::procs::proc_id(own_pid).map(|p| p.start as i64);

    crate::db::set_auto_json(&tx, task_id, Some(&aj.to_json()))?;
    crate::db::set_auto_state(&tx, task_id, Some("running"))?;
    crate::db::set_auto_driver(&tx, task_id, Some(own_pid as i64), own_start)?;
    crate::db::clear_auto_stop(&tx, task_id)?;
    tx.commit()?;
    Ok(aj)
}

/// Trả task: ghi auto_state, auto_json và xóa pid của driver — một câu lệnh,
/// chỉ khi hàng vẫn thuộc tiến trình này (`auto_pid` = pid của mình). Hàng đã
/// đổi chủ (driver khác nhận, Dừng hẳn) thì không ghi gì và in một dòng.
pub fn release(conn: &Connection, task_id: i64, state: &str, aj: &AutoJson) -> BrainResult<()> {
    let own_pid = std::process::id() as i64;
    let changed = conn.execute(
        "UPDATE tasks SET auto_json = ?1, auto_state = ?2, auto_pid = NULL, auto_pid_start = NULL, updated_at = ?5 WHERE id = ?3 AND auto_pid IS ?4",
        rusqlite::params![aj.to_json(), state, task_id, own_pid, crate::db::now_ms()],
    )?;
    if changed == 0 {
        crate::term::emit(&format!(
            "Autopilot: task {task_id} đã đổi trạng thái bởi tiến trình khác — không ghi đè"
        ));
    }
    Ok(())
}

/// Dừng hẳn autopilot: auto_state `ended`, xóa pid và cờ dừng — chỉ khi
/// auto_state vẫn là `expect_state` và không có driver giữ task. Trả true khi
/// đã ghi (false = trạng thái đã đổi giữa lúc đọc và lúc ghi).
pub fn end_autopilot(
    conn: &Connection,
    task_id: i64,
    expect_state: &str,
    aj: &AutoJson,
) -> BrainResult<bool> {
    let changed = conn.execute(
        "UPDATE tasks SET auto_json = ?1, auto_state = 'ended', auto_pid = NULL, auto_pid_start = NULL, auto_stop_at = NULL, auto_stop_kind = NULL, updated_at = ?4 WHERE id = ?2 AND auto_state = ?3 AND auto_pid IS NULL",
        rusqlite::params![aj.to_json(), task_id, expect_state, crate::db::now_ms()],
    )?;
    Ok(changed == 1)
}

/// Hoàn lại duyệt plan sau khi autopilot không bắt đầu được — chỉ khi
/// approved_commit vẫn là commit mà yêu cầu này vừa ghi (`written_commit`), để
/// không xóa một lần duyệt khác ghi trong lúc đó. `prev` = (approved_at,
/// approved_commit) trước khi ghi; None = xóa duyệt. Trả true khi đã ghi.
pub fn revert_approval(
    conn: &Connection,
    task_id: i64,
    written_commit: &str,
    prev: Option<(i64, &str)>,
) -> BrainResult<bool> {
    let (at, commit) = match prev {
        Some((at, c)) => (Some(at), Some(c)),
        None => (None, None),
    };
    let changed = conn.execute(
        "UPDATE tasks SET approved_at = ?1, approved_commit = ?2 WHERE id = ?3 AND approved_commit = ?4",
        rusqlite::params![at, commit, task_id, written_commit],
    )?;
    if changed == 1 {
        let _ = conn.execute(
            "DELETE FROM plan_approvals WHERE id = (SELECT id FROM plan_approvals WHERE task_id = ?1 AND \"commit\" = ?2 ORDER BY id DESC LIMIT 1)",
            rusqlite::params![task_id, written_commit],
        );
    }
    Ok(changed == 1)
}

/// Slug và profile của dự án chứa task.
pub fn task_project(conn: &Connection, task_id: i64) -> BrainResult<Option<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT projects.slug, projects.profile FROM tasks JOIN projects ON tasks.project_id = projects.id WHERE tasks.id = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![task_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some((r.get(0)?, r.get(1)?)))
    } else {
        Ok(None)
    }
}

/// Kiểm tra trước khi ghi duyệt plan; Ok = sha của commit plan hiện tại.
pub fn approve_check(
    conn: &Connection,
    task: &TaskRow,
    root: &Path,
    pricing: &BTreeMap<String, Price>,
    protocol: &crate::protocol_paths::ProtocolPaths,
) -> Result<String, String> {
    let db_err = |e: BrainError| e.message;
    let auto = crate::db::task_auto(conn, task.id)
        .map_err(db_err)?
        .unwrap_or_default();
    if auto.workflow_json.is_none() {
        return Err(
            "Task tạo trước Phase 5.2 — không có cổng duyệt để ghi; bấm Chạy Dev để duyệt như trước."
                .to_string(),
        );
    }
    if driver_alive(&auto) {
        return Err("Autopilot đang chạy task này — chờ autopilot dừng".to_string());
    }
    if let Some(r) = crate::db::running_run_for_task(conn, task.id).map_err(db_err)? {
        if run_alive(&r) {
            return Err("Đang có run chạy — chờ run xong hoặc bấm Kill".to_string());
        }
    }
    let plan_commit = match crate::gitops::plan_commit_on_branch(
        root,
        &task.branch,
        &task.slug,
        task.base_commit.as_deref(),
    ) {
        Err(()) => {
            return Err(format!(
                "Không đọc được lịch sử git của nhánh {}",
                task.branch
            ))
        }
        Ok(None) => {
            return Err(format!(
                "Chưa có commit '{}: plan' trên nhánh {}",
                task.slug, task.branch
            ))
        }
        Ok(Some(pc)) => pc,
    };
    if crate::gitops::current_branch(root).as_deref() == Some(task.branch.as_str()) {
        let dirty = crate::gitops::changed_paths(root).map_err(db_err)?;
        if dirty.contains(&protocol.plan) {
            return Err(format!(
                "Plan có thay đổi chưa commit — commit với message '{}: plan' rồi duyệt",
                task.slug
            ));
        }
    }
    if auto.approved_commit.as_deref() == Some(plan_commit.as_str()) {
        let at = auto
            .approved_at
            .map(crate::autopilot::view::fmt_hhmm)
            .unwrap_or_else(|| "--:--".to_string());
        return Err(format!(
            "Plan hiện tại đã được duyệt lúc {at} — không duyệt lại"
        ));
    }
    let plan_text = match crate::gitops::git(root, &["show", &protocol.plan_at(&plan_commit)]) {
        Ok(t) => t,
        Err(_) => {
            return Err(format!(
                "Không đọc được {} từ commit của plan",
                protocol.plan
            ))
        }
    };
    let parsed_plan = crate::planfile::parse_plan(&plan_text);
    let grants = crate::planfile::run_grants(&parsed_plan);
    let grant_errors = crate::planfile::validate_grants(&grants, Some(root));
    if !grant_errors.is_empty() {
        let list = grant_errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("Plan có quyền không hợp lệ:\n{list}"));
    }

    let wf: Option<crate::workflows::Workflow> = auto
        .workflow_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok());
    let continues = wf.as_ref().is_some_and(|w| w.auto)
        && auto.auto_state.as_deref() != Some("ended");

    if continues {
        let unhandled = parsed_plan
            .sections
            .iter()
            .find(|(k, _)| k.starts_with("Vướng mắc"))
            .map(|(_, v)| crate::planfile::unhandled_blocker_lines(v))
            .unwrap_or_default();
        if !unhandled.is_empty() {
            let n = unhandled.len();
            let preview = unhandled.join("; ");
            return Err(format!("Plan còn {n} vướng mắc chưa đánh dấu [x]: {preview}"));
        }
        if let Some(ref w) = wf {
            let aj = crate::autopilot::decide::AutoJson::parse(auto.auto_json.as_deref());
            let runs = crate::db::runs_for_task(conn, task.id).map_err(db_err)?;
            let now_ms = chrono::Utc::now().timestamp_millis();
            if let Some(reason) = crate::pricing::budget_exhausted_reason(&runs, pricing, w, &aj, now_ms) {
                return Err(reason);
            }
        }
    }

    Ok(plan_commit)
}

/// Ghi duyệt plan cho commit hiện tại; trả thời điểm (ms) đã ghi.
///
/// `skip_build` ghi `auto_json.skip_build_once`: duyệt "thẳng QA" bật cờ, mọi lần duyệt khác
/// tắt cờ. Cờ giữ nguyên tới lần duyệt sau, để sau khi QA chạy autopilot không quay lại implement.
pub fn approve_record(
    conn: &Connection,
    task_id: i64,
    plan_commit: &str,
    source: &str,
    skip_build: bool,
) -> BrainResult<i64> {
    let now = crate::db::now_ms();
    crate::db::set_task_approval(conn, task_id, now, plan_commit)?;
    crate::db::insert_plan_approval(conn, task_id, plan_commit, now, source)?;
    let raw: Option<String> = conn
        .query_row("SELECT auto_json FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))
        .optional()?
        .flatten();
    let mut aj = AutoJson::parse(raw.as_deref());
    if aj.skip_build_once != skip_build {
        aj.skip_build_once = skip_build;
        crate::db::set_auto_json(conn, task_id, Some(&aj.to_json()))?;
    }
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open, project_row_id, sync_projects, task_auto, upsert_task};
    use crate::projects::Project;

    fn fresh_task() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = open(&dir.path().join("brain.db")).expect("open db");
        let project = Project {
            id: "demo".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            name_vi: "Demo".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        };
        sync_projects(&conn, &[project]).expect("sync projects");
        let project_id = project_row_id(&conn, "demo").expect("project id");
        let task_id =
            upsert_task(&conn, project_id, "t", "T", "t", Some("main"), None).expect("upsert task");
        (dir, conn, task_id)
    }

    fn set_row(conn: &Connection, task_id: i64, state: &str, pid: Option<i64>) {
        conn.execute(
            "UPDATE tasks SET auto_state = ?1, auto_pid = ?2, auto_pid_start = ?3, auto_stop_at = 5, auto_stop_kind = 'soft' WHERE id = ?4",
            rusqlite::params![state, pid, pid.map(|_| 7i64), task_id],
        )
        .expect("set row");
    }

    fn auto_of(conn: &Connection, task_id: i64) -> TaskAuto {
        task_auto(conn, task_id).expect("task_auto").expect("row")
    }

    #[test]
    fn release_only_own_row() {
        let (_dir, conn, id) = fresh_task();
        let own = std::process::id() as i64;
        set_row(&conn, id, "running", Some(own));
        release(&conn, id, "gate", &AutoJson::default()).expect("release own");
        let a = auto_of(&conn, id);
        assert_eq!(a.auto_state.as_deref(), Some("gate"));
        assert_eq!(a.auto_pid, None);
        assert_eq!(a.auto_pid_start, None);

        set_row(&conn, id, "running", Some(1));
        release(&conn, id, "gate", &AutoJson::default()).expect("release other");
        let a = auto_of(&conn, id);
        assert_eq!(a.auto_state.as_deref(), Some("running"));
        assert_eq!(a.auto_pid, Some(1));
        assert_eq!(a.auto_pid_start, Some(7));
    }

    #[test]
    fn gate_hold_counts_only_waiting_gate_of_other_tasks() {
        let (_dir, conn, a) = fresh_task();
        let pid = project_row_id(&conn, "demo").expect("project id");
        let b = upsert_task(&conn, pid, "b", "B", "task/b", None, None).expect("task b");
        // Nothing at the gate: free, whatever the option says.
        assert_eq!(project_busy_reason(&conn, "demo", "default", Some(b), true).unwrap(), None);
        conn.execute("UPDATE tasks SET auto_state = 'gate' WHERE id = ?1", [a]).unwrap();
        // Task a waits at the gate: b (and a new task) are held, a itself is not.
        let held = project_busy_reason(&conn, "demo", "default", Some(b), true)
            .unwrap()
            .expect("held");
        assert!(held.contains(&format!("task {a}")), "{held}");
        assert!(held.contains(crate::projects::HOLD_AT_GATE_KEY), "{held}");
        assert!(project_busy_reason(&conn, "demo", "default", None, true).unwrap().is_some());
        assert_eq!(project_busy_reason(&conn, "demo", "default", Some(a), true).unwrap(), None);
        // Option off: the old behaviour.
        assert_eq!(project_busy_reason(&conn, "demo", "default", Some(b), false).unwrap(), None);
        // Another project is never held.
        assert_eq!(gate_hold_reason(&conn, "other", None).unwrap(), None);
        // Two gated tasks do not hold each other.
        conn.execute("UPDATE tasks SET auto_state = 'gate' WHERE id = ?1", [b]).unwrap();
        assert_eq!(project_busy_reason(&conn, "demo", "default", Some(b), true).unwrap(), None);
        conn.execute("UPDATE tasks SET auto_state = NULL WHERE id = ?1", [b]).unwrap();
        // An approval recorded without running on leaves `gate`, and frees the project.
        crate::db::set_task_approval(&conn, a, crate::db::now_ms(), "c1").unwrap();
        assert_eq!(project_busy_reason(&conn, "demo", "default", Some(b), true).unwrap(), None);
    }

    #[test]
    fn approve_branch_decisions() {
        let none: Vec<String> = vec![];
        let dirty = vec!["b.txt".to_string(), "a.txt".to_string()];
        assert_eq!(
            approve_branch(true, "task/x", Some("task/x"), true, &dirty, Some("busy")),
            ApproveBranch::OnBranch
        );
        assert_eq!(
            approve_branch(true, "task/x", Some("main"), true, &none, None),
            ApproveBranch::Checkout
        );
        let mismatch = "Không ở nhánh task 'task/x' (nhánh hiện tại: 'main') — không đoán";
        assert_eq!(
            approve_branch(false, "task/x", Some("main"), true, &none, None),
            ApproveBranch::Refuse(mismatch.to_string())
        );
        assert_eq!(
            approve_branch(true, "task/x", Some("main"), false, &none, None),
            ApproveBranch::Refuse(mismatch.to_string())
        );
        match approve_branch(true, "task/x", Some("main"), true, &dirty, None) {
            ApproveBranch::Refuse(r) => {
                assert!(r.starts_with(mismatch), "{r}");
                assert!(r.ends_with("a.txt\nb.txt"), "{r}");
            }
            other => panic!("{other:?}"),
        }
        match approve_branch(true, "task/x", Some("main"), true, &none, Some("Dự án bận")) {
            ApproveBranch::Refuse(r) => assert!(r.ends_with("Dự án bận"), "{r}"),
            other => panic!("{other:?}"),
        }
    }

    fn approval_of(conn: &Connection, task_id: i64) -> (Option<i64>, Option<String>) {
        let a = auto_of(conn, task_id);
        (a.approved_at, a.approved_commit)
    }

    #[test]
    fn revert_approval_conditional() {
        let (_dir, conn, id) = fresh_task();
        // Yêu cầu này vừa ghi c2 (trước đó c1 lúc 100): hoàn lại c1.
        crate::db::set_task_approval(&conn, id, 200, "c2").expect("approve");
        assert!(revert_approval(&conn, id, "c2", Some((100, "c1"))).expect("revert"));
        assert_eq!(approval_of(&conn, id), (Some(100), Some("c1".to_string())));

        // Một tiến trình khác duyệt c3 trong lúc đó: giữ c3.
        crate::db::set_task_approval(&conn, id, 300, "c3").expect("approve");
        assert!(!revert_approval(&conn, id, "c2", Some((100, "c1"))).expect("revert"));
        assert_eq!(approval_of(&conn, id), (Some(300), Some("c3".to_string())));

        // Không có duyệt trước đó: xóa về NULL.
        crate::db::set_task_approval(&conn, id, 400, "c4").expect("approve");
        assert!(revert_approval(&conn, id, "c4", None).expect("revert"));
        assert_eq!(approval_of(&conn, id), (None, None));
    }

    #[test]
    fn end_autopilot_conditional() {
        let (_dir, conn, id) = fresh_task();
        set_row(&conn, id, "stopped", None);
        assert!(end_autopilot(&conn, id, "stopped", &AutoJson::default()).expect("end"));
        let a = auto_of(&conn, id);
        assert_eq!(a.auto_state.as_deref(), Some("ended"));
        assert_eq!(a.auto_pid, None);
        assert_eq!(a.auto_stop_at, None);
        assert_eq!(a.auto_stop_kind, None);

        // Một driver đã nhận task giữa lúc đọc và lúc ghi: không ghi gì.
        set_row(&conn, id, "running", Some(1));
        assert!(!end_autopilot(&conn, id, "stopped", &AutoJson::default()).expect("end"));
        let a = auto_of(&conn, id);
        assert_eq!(a.auto_state.as_deref(), Some("running"));
        assert_eq!(a.auto_pid, Some(1));
        assert_eq!(a.auto_stop_kind.as_deref(), Some("soft"));

        // auto_state đúng nhưng còn pid: cũng không ghi.
        set_row(&conn, id, "stopped", Some(1));
        assert!(!end_autopilot(&conn, id, "stopped", &AutoJson::default()).expect("end"));
        assert_eq!(auto_of(&conn, id).auto_state.as_deref(), Some("stopped"));

        set_row(&conn, id, "gate", None);
        assert!(end_autopilot(&conn, id, "gate", &AutoJson::default()).expect("end"));
        assert_eq!(auto_of(&conn, id).auto_state.as_deref(), Some("ended"));
    }

    #[test]
    fn more_round_grant_lifts_cap() {
        assert_eq!(more_round_grant(Some(3), Some(3)), 1);
        // G-01: Trả về Lead chạy vòng 4 với trần 3 → cấp 2 để trần thành 5.
        assert_eq!(more_round_grant(Some(4), Some(3)), 2);
        assert_eq!(more_round_grant(Some(4), Some(4)), 1);
        assert_eq!(more_round_grant(Some(2), Some(3)), 1);
        assert_eq!(more_round_grant(None, Some(3)), 1);
        assert_eq!(more_round_grant(Some(4), None), 1);
    }

    #[test]
    fn approve_check_rejects_invalid_grants() {
        let (dir, conn, id) = fresh_task();
        conn.execute("UPDATE tasks SET workflow_json = '{}' WHERE id = ?1", rusqlite::params![id]).unwrap();
        let root = dir.path();
        let _ = std::process::Command::new("git").args(["init"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["checkout", "-b", "t"]).current_dir(root).output();

        let plan_content = "# Kế hoạch\n\n## Lệnh được chạy\n- `git add foo/*`\n";
        std::fs::write(root.join(".plan.md"), plan_content).unwrap();
        let _ = std::process::Command::new("git").args(["add", ".plan.md"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["commit", "-m", "t: plan"]).current_dir(root).output();

        let task = crate::db::get_task(&conn, id).unwrap().unwrap();
        let err = approve_check(&conn, &task, root, &crate::config::default_pricing(), &Default::default()).unwrap_err();
        assert!(err.contains("Plan có quyền không hợp lệ"), "{err}");
        assert!(err.contains("git add foo/*"), "{err}");
    }

    #[test]
    fn approve_check_blocker_checkboxes() {
        let (dir, conn, id) = fresh_task();
        let root = dir.path();
        let wf = crate::workflows::Workflow {
            name: "mac-dinh".to_string(),
            kind: "code".to_string(),
            stages: vec!["plan".to_string(), "gate".to_string(), "dev".to_string()],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 10.0,
            auto: true,
            review: false,
            source: "builtin".to_string(),
            timeouts: Default::default(),
            finalize_on_timeout: Default::default(),
        };
        conn.execute(
            "UPDATE tasks SET workflow_json = ?1 WHERE id = ?2",
            rusqlite::params![wf.to_snapshot(), id],
        )
        .unwrap();

        let _ = std::process::Command::new("git").args(["init"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["checkout", "-b", "t"]).current_dir(root).output();

        // 1. Plan with unhandled blocker
        let plan_with_blocker = "# Plan: t\n\n## Vướng mắc\n- [ ] Cần token staging\n";
        std::fs::write(root.join(".plan.md"), plan_with_blocker).unwrap();
        let _ = std::process::Command::new("git").args(["add", ".plan.md"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["commit", "-m", "t: plan"]).current_dir(root).output();

        let task = crate::db::get_task(&conn, id).unwrap().unwrap();
        let err = approve_check(&conn, &task, root, &crate::config::default_pricing(), &Default::default()).unwrap_err();
        assert!(err.contains("vướng mắc chưa đánh dấu [x]"), "{err}");

        // 2. Resolve blocker with [x]
        let plan_resolved = "# Plan: t\n\n## Vướng mắc\n- [x] Cần token staging\n";
        std::fs::write(root.join(".plan.md"), plan_resolved).unwrap();
        let _ = std::process::Command::new("git").args(["add", ".plan.md"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["commit", "-m", "t: plan"]).current_dir(root).output();

        let res = approve_check(&conn, &task, root, &crate::config::default_pricing(), &Default::default());
        assert!(res.is_ok(), "Expected Ok but got: {:?}", res.err());
    }

    #[test]
    fn approve_check_budget_with_configured_pricing() {
        let (dir, conn, id) = fresh_task();
        let root = dir.path();
        let wf = crate::workflows::Workflow {
            name: "mac-dinh".to_string(),
            kind: "code".to_string(),
            stages: vec!["plan".to_string(), "gate".to_string(), "dev".to_string()],
            loops: vec![],
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 10.0,
            auto: true,
            review: false,
            source: "builtin".to_string(),
            timeouts: Default::default(),
            finalize_on_timeout: Default::default(),
        };
        conn.execute(
            "UPDATE tasks SET workflow_json = ?1 WHERE id = ?2",
            rusqlite::params![wf.to_snapshot(), id],
        )
        .unwrap();

        let _ = std::process::Command::new("git").args(["init"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["checkout", "-b", "t"]).current_dir(root).output();

        let plan_content = "# Plan: t\n\n## Thay đổi\n| Thao tác | Đường dẫn | Mô tả |\n|---|---|---|\n| sửa | a.txt | mô tả |\n";
        std::fs::write(root.join(".plan.md"), plan_content).unwrap();
        let _ = std::process::Command::new("git").args(["add", ".plan.md"]).current_dir(root).output();
        let _ = std::process::Command::new("git").args(["commit", "-m", "t: plan"]).current_dir(root).output();

        let pid = crate::db::project_row_id(&conn, "demo").unwrap();
        let new_run = crate::db::NewRun {
            task_id: Some(id),
            project_id: pid,
            role: "lead".to_string(),
            profile: "default".to_string(),
            model: Some("custom-model".to_string()),
            pid: 1,
            pgid: 1,
            pid_start: 1,
            base_commit: None,
            run_dir: None,
        };
        let run_id = crate::db::insert_run(&conn, &new_run).unwrap();
        // Set run ended and 100_000 output tokens
        conn.execute(
            "UPDATE runs SET status = 'succeeded', started_at = 1000, ended_at = 2000, output_tokens = 100000, usage_source = 'partial' WHERE id = ?1",
            rusqlite::params![run_id],
        ).unwrap();

        // Custom pricing where custom-model costs $200 per 1M output tokens (so 100k tokens = $20.0 > $10.0 limit)
        let mut custom_pricing = std::collections::BTreeMap::new();
        custom_pricing.insert(
            "custom-model".to_string(),
            Price {
                input: 10.0,
                output: 200.0,
                cache_read: 1.0,
                cache_write: 5.0,
            },
        );

        let task = crate::db::get_task(&conn, id).unwrap().unwrap();
        // Default pricing does not have custom-model, so custom pricing proves it's using configured pricing
        let err = approve_check(&conn, &task, root, &custom_pricing, &Default::default()).unwrap_err();
        assert!(err.contains("Hết ngân sách"), "{err}");

        // Now add budget extension
        let aj = AutoJson {
            budget_extra_usd: 50.0,
            ..Default::default()
        };
        conn.execute(
            "UPDATE tasks SET auto_json = ?1 WHERE id = ?2",
            rusqlite::params![aj.to_json(), id],
        )
        .unwrap();

        let ok = approve_check(&conn, &task, root, &custom_pricing, &Default::default());
        assert!(ok.is_ok(), "Expected Ok after budget extension: {:?}", ok.err());
    }

    /// Duyệt "thẳng QA" bật `skip_build_once`; lần duyệt sau (bất kỳ kiểu khác) tắt cờ.
    /// Mỗi lần duyệt thêm một dòng `plan_approvals`; hoàn duyệt xoá dòng của lần đó (B2, B3).
    #[test]
    fn approve_record_sets_skip_build_and_history() {
        let (_dir, conn, task_id) = fresh_task();
        let skip = |conn: &Connection| {
            AutoJson::parse(task_auto(conn, task_id).unwrap().unwrap().auto_json.as_deref()).skip_build_once
        };
        let count = |conn: &Connection| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM plan_approvals WHERE task_id = ?1",
                [task_id],
                |r| r.get(0),
            )
            .unwrap()
        };

        approve_record(&conn, task_id, "c1", "ui", true).unwrap();
        assert!(skip(&conn));
        assert_eq!(count(&conn), 1);

        approve_record(&conn, task_id, "c2", "cli", false).unwrap();
        assert!(!skip(&conn));
        assert_eq!(count(&conn), 2);
        assert_eq!(
            crate::db::latest_different_plan_approval(&conn, task_id, "c2").unwrap().as_deref(),
            Some("c1")
        );
    }
}
