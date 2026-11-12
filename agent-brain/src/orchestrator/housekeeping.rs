//! Housekeeping: tự dọn worktree của task đã xong và nhánh task đã merge, và dọn sớm khi ổ đĩa
//! chứa worktree gần đầy. Chạy trong tick của scheduler (kể cả khi `orchestrator.enabled = false`),
//! cấu hình ở khối `housekeeping` của config.json.
//!
//! Luật (đã chốt với người dùng):
//! 1. Nhánh task ĐÃ merge thì xoá, chỉ bằng `git branch -d` (không bao giờ `-D`).
//! 2. Nhánh CHƯA merge không bao giờ tự xoá.
//! 3. Worktree sạch của task xong mà nhánh chưa merge: dọn worktree (giữ nhánh) khi quá
//!    `keep_unmerged_days` ngày hoặc ổ đĩa còn dưới `min_free_gb`.
//! 4. Không xoá nhánh đang là `base_branch` của một task chưa xong, nhánh đang checkout, hay
//!    nhánh không có trong bảng `tasks`.
//!
//! Không dùng `--force`, `-D`, `git clean` hay `rm`: việc dọn worktree đi qua
//! `workspace::cleanup_task` (git từ chối worktree có thay đổi chưa commit).

use crate::config::{Config, Housekeeping};
use crate::dashboard::AppState;
use crate::db::TaskRow;
use crate::error::BrainResult;
use crate::projects::Project;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const DAY_MS: i64 = 86_400_000;

/// Thời điểm (ms) lần dọn gần nhất; 0 = chưa dọn lần nào kể từ khi serve chạy.
fn last_run() -> &'static Mutex<i64> {
    static LAST: OnceLock<Mutex<i64>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(0))
}

/// Cảnh báo đã ghi (task, lý do) — chỉ ghi một lần mỗi lần chạy serve.
fn warned() -> &'static Mutex<HashSet<(i64, String)>> {
    static WARNED: OnceLock<Mutex<HashSet<(i64, String)>>> = OnceLock::new();
    WARNED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Ghi cảnh báo của task một lần cho mỗi lý do (dòng đầu của lý do).
fn warn_once(task_id: i64, reason: &str) {
    let first = reason.lines().next().unwrap_or("").trim().to_string();
    let fresh = warned()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert((task_id, first.clone()));
    if fresh {
        crate::term::emit(&format!("Dọn tự động: bỏ qua task {task_id} — {first}"));
    }
}

/// True khi đã đến lúc dọn: chưa dọn lần nào (`last == 0`) hoặc đã qua `interval_minutes`.
pub fn due(last_ms: i64, now_ms: i64, interval_minutes: u64) -> bool {
    last_ms == 0 || now_ms.saturating_sub(last_ms) >= interval_minutes as i64 * 60_000
}

/// Phần "Available" (KiB) của đầu ra `df -Pk` (dòng dữ liệu cuối; cột thứ tư).
pub fn parse_df_available_kb(out: &str) -> Option<u64> {
    let line = out.lines().rev().find(|l| !l.trim().is_empty())?;
    line.split_whitespace().nth(3)?.parse::<u64>().ok()
}

/// Dung lượng trống (GB, làm tròn xuống) của hệ thống tệp chứa `dir`, lấy bằng `df -Pk`.
/// Thư mục chưa có thì hỏi thư mục cha gần nhất đang có. `None` khi không đọc được.
pub fn free_gb_of(dir: &Path) -> Option<u64> {
    let mut probe = dir;
    while !probe.exists() {
        probe = probe.parent()?;
    }
    let out = std::process::Command::new("df")
        .args(["-Pk"])
        .arg(probe)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let kb = parse_df_available_kb(&String::from_utf8_lossy(&out.stdout))?;
    Some(kb / (1024 * 1024))
}

/// Gọi từ tick của scheduler: dọn nếu `housekeeping.enabled` và đã đến hạn.
pub fn run_if_due(st: &AppState, conn: &Connection, now_ms: i64) -> BrainResult<Vec<String>> {
    let hk = &st.cfg.housekeeping;
    if !hk.enabled {
        return Ok(Vec::new());
    }
    {
        let mut last = last_run().lock().unwrap_or_else(|e| e.into_inner());
        if !due(*last, now_ms, hk.interval_minutes) {
            return Ok(Vec::new());
        }
        *last = now_ms;
    }
    let free = free_gb_of(&crate::workspace::worktrees_dir());
    tick(st, conn, now_ms, free)
}

/// Một lượt dọn của mọi dự án chế độ worktree trong map của `st`. `free_gb`: dung lượng trống của
/// ổ chứa worktree (`None` = không biết, không dọn theo dung lượng). Trả các dòng đã làm.
pub fn tick(
    st: &AppState,
    conn: &Connection,
    now_ms: i64,
    free_gb: Option<u64>,
) -> BrainResult<Vec<String>> {
    if !st.cfg.housekeeping.enabled {
        return Ok(Vec::new());
    }
    let map = crate::projects::load_map(&st.map_path)?;
    let projects = crate::projects::projects_of(&map);
    // Job đang chạy mà hàng run chưa kịp ghi vào DB.
    let busy: HashSet<i64> = st
        .jobs
        .list()
        .into_iter()
        .filter(|j| j.state == "running")
        .filter_map(|j| j.task_id)
        .collect();
    tick_with(&st.cfg, &projects, conn, now_ms, free_gb, &busy)
}

/// `tick` không cần `AppState`: cấu hình, danh sách dự án và tập task đang bận cho sẵn.
pub fn tick_with(
    cfg: &Config,
    projects: &[Project],
    conn: &Connection,
    now_ms: i64,
    free_gb: Option<u64>,
    busy_tasks: &HashSet<i64>,
) -> BrainResult<Vec<String>> {
    let hk = &cfg.housekeeping;
    let mut done = Vec::new();
    if !hk.enabled {
        return Ok(done);
    }
    for project in projects {
        if !project.workspace.is_worktree() {
            continue;
        }
        match tick_project(cfg, hk, project, conn, now_ms, free_gb, busy_tasks) {
            Ok(lines) => done.extend(lines),
            Err(e) => crate::term::emit(&format!(
                "Dọn tự động: dự án {} lỗi — {}",
                project.id, e.message
            )),
        }
    }
    Ok(done)
}

fn has_workdir(task: &TaskRow) -> bool {
    task.workdir.as_deref().is_some_and(|w| !w.trim().is_empty())
}

fn tick_project(
    cfg: &Config,
    hk: &Housekeeping,
    project: &Project,
    conn: &Connection,
    now_ms: i64,
    free_gb: Option<u64>,
    busy_tasks: &HashSet<i64>,
) -> BrainResult<Vec<String>> {
    let mut done = Vec::new();
    let Ok(pid) = crate::db::project_row_id(conn, &project.id) else {
        return Ok(done);
    };
    let tasks = crate::db::tasks_of_project(conn, pid)?;
    if tasks.is_empty() {
        return Ok(done);
    }
    let Ok(root) = crate::projects::require_runnable(project, cfg) else {
        return Ok(done);
    };
    let low_disk = hk.min_free_gb > 0 && free_gb.is_some_and(|f| f < hk.min_free_gb);
    let max_age_ms = hk.keep_unmerged_days as i64 * DAY_MS;

    for task in tasks.iter().filter(|t| has_workdir(t)) {
        // Chỉ task đã xong; task chưa xong không bao giờ bị đụng.
        if !crate::tasks::is_success_status(&task.status) || busy_tasks.contains(&task.id) {
            continue;
        }
        if crate::workspace::task_busy(conn, task)? {
            continue;
        }
        let merged = crate::workspace::merge_base_of(project, task)
            .is_some_and(|base| crate::gitops::is_merged_into(&root, &task.branch, &base));
        let confirmed = if merged {
            false
        } else if now_ms.saturating_sub(task.updated_at) > max_age_ms || low_disk {
            true // chỉ dọn worktree; cleanup_task không bao giờ xoá nhánh
        } else {
            continue;
        };
        match crate::workspace::cleanup_task(conn, project, &root, task, confirmed)? {
            Ok(msg) => {
                crate::term::emit(&format!("Dọn tự động: {msg}"));
                done.push(msg);
            }
            Err(reason) => warn_once(task.id, &reason),
        }
    }

    if hk.delete_merged_branches {
        let sweep = sweep_merged_branches(conn, project, &root)?;
        for line in &sweep.deleted {
            crate::term::emit(&format!("Dọn tự động: {line}"));
        }
        for (id, reason) in &sweep.refused {
            warn_once(*id, reason);
        }
        done.extend(sweep.deleted);
    }
    Ok(done)
}

/// Kết quả một lượt xoá nhánh đã merge của một dự án.
#[derive(Debug, Default, Clone)]
pub struct BranchSweep {
    /// Dòng mô tả từng nhánh đã xoá.
    pub deleted: Vec<String>,
    /// (id task, lý do) của nhánh mà git từ chối xoá.
    pub refused: Vec<(i64, String)>,
}

/// Tiền tố nhánh do agent-brain tạo cho task.
fn is_task_branch(branch: &str) -> bool {
    branch.starts_with("task/") || branch.starts_with("analysis/")
}

/// Xoá nhánh của task xong KHÔNG còn worktree khi nhánh `task/*` hoặc `analysis/*` ghi trong DB
/// còn tồn tại, đã merge vào nhánh gốc, không đang checkout ở repo chính, và không task chưa xong
/// nào dùng nó làm `base_branch`. Chỉ `git branch -d -- <nhánh>`: git từ chối thì ghi vào
/// `refused`. Không đụng nhánh không có trong bảng `tasks`.
pub fn sweep_merged_branches(
    conn: &Connection,
    project: &Project,
    root: &Path,
) -> BrainResult<BranchSweep> {
    let mut out = BranchSweep::default();
    let Ok(pid) = crate::db::project_row_id(conn, &project.id) else {
        return Ok(out);
    };
    let tasks = crate::db::tasks_of_project(conn, pid)?;
    let current = crate::gitops::current_branch(root);
    for task in &tasks {
        let branch = task.branch.as_str();
        if !crate::tasks::is_success_status(&task.status)
            || has_workdir(task)
            || !is_task_branch(branch)
            || !crate::gitops::valid_branch_name_syntax(branch)
            || !crate::gitops::branch_exists(root, branch)
        {
            continue;
        }
        let Some(base) = crate::workspace::merge_base_of(project, task) else {
            continue;
        };
        if base == branch || !crate::gitops::is_merged_into(root, branch, &base) {
            continue;
        }
        if current.as_deref() == Some(branch) {
            continue;
        }
        let is_base_of_unfinished = tasks.iter().any(|o| {
            !crate::tasks::is_success_status(&o.status)
                && o.base_branch.as_deref().map(str::trim) == Some(branch)
        });
        if is_base_of_unfinished {
            continue;
        }
        match crate::gitops::git(root, &["branch", "-d", "--", branch]) {
            Ok(_) => out.deleted.push(format!(
                "đã xoá nhánh '{branch}' của task {} (đã merge vào '{base}').",
                task.id
            )),
            Err(e) => out
                .refused
                .push((task.id, format!("git branch -d '{branch}' từ chối: {}", e.message))),
        }
    }
    Ok(out)
}

/// `sweep_merged_branches` trả về các dòng cho người đọc: nhánh đã xoá, rồi lý do git từ chối.
pub fn delete_merged_branches(conn: &Connection, project: &Project, root: &Path) -> Vec<String> {
    match sweep_merged_branches(conn, project, root) {
        Ok(s) => s
            .deleted
            .into_iter()
            .chain(s.refused.into_iter().map(|(id, r)| format!("task {id}: {r}")))
            .collect(),
        Err(e) => vec![format!("Không đọc được task của dự án {}: {}", project.id, e.message)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_when_never_run_or_interval_passed() {
        assert!(due(0, 1_000, 10));
        assert!(!due(1_000, 1_000 + 9 * 60_000, 10));
        assert!(due(1_000, 1_000 + 10 * 60_000, 10));
        // a clock that went backwards never makes it due early
        assert!(!due(5_000_000, 1_000, 10));
    }

    #[test]
    fn parses_df_output() {
        let out = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sda2        455000000 251000000 181000000      59% /mnt/data\n";
        assert_eq!(parse_df_available_kb(out), Some(181_000_000));
        assert_eq!(parse_df_available_kb(""), None);
        assert_eq!(parse_df_available_kb("Filesystem\n/dev/x a b c d\n"), None);
    }

    #[test]
    fn free_gb_of_a_missing_dir_asks_its_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("no/such/dir");
        assert!(free_gb_of(&missing).is_some());
    }
}
