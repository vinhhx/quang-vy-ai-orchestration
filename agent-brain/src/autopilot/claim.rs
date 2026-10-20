//! Nhận task cho driver autopilot (một driver cho một task) và trả task khi dừng.
//!
//! Mọi kiểm tra "đang bận" dựa trên cặp (pid, thời điểm khởi động) qua
//! `procs::alive`, nên một driver đã chết để lại hàng `running` vẫn nhận lại được.

use crate::autopilot::decide::{AutoJson, Replan, StopReason};
use crate::db::{DriverRow, RunRow, TaskAuto};
use crate::error::{BrainError, BrainResult};
use crate::procs::ProcId;
use rusqlite::Connection;

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

/// Dự án (hoặc profile personal) đang có run của task khác hoặc driver khác.
pub fn project_busy_reason(
    conn: &Connection,
    project_slug: &str,
    profile: &str,
    exclude_task: Option<i64>,
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
    live_driver_reason(conn, project_slug, profile, exclude_task)
}

/// Tùy chọn khi nhận task: `Chạy thêm 1 vòng` và yêu cầu lập lại plan.
#[derive(Debug, Clone, Default)]
pub struct ClaimOpts {
    pub more_round: bool,
    pub replan: Option<Replan>,
}

/// Nhận task cho driver này trong một giao dịch BEGIN IMMEDIATE.
pub fn claim(
    conn: &mut Connection,
    task_id: i64,
    project_slug: &str,
    profile: &str,
    opts: &ClaimOpts,
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
    if let Some(reason) = project_busy_reason(&tx, project_slug, profile, Some(task_id))? {
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
                    *aj.grants.entry(key).or_insert(0) += 1;
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

/// Trả task: ghi auto_state, auto_json và xóa pid của driver.
pub fn release(conn: &Connection, task_id: i64, state: &str, aj: &AutoJson) -> BrainResult<()> {
    crate::db::set_auto_json(conn, task_id, Some(&aj.to_json()))?;
    crate::db::set_auto_state(conn, task_id, Some(state))?;
    crate::db::set_auto_driver(conn, task_id, None, None)?;
    Ok(())
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
