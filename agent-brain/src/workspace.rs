//! Per-task git worktrees (D2.2, D2.4, D2.5).
//!
//! A project with `workspace: "worktree"` gives every task its own git worktree at
//! `<data_dir>/worktrees/<project_id>/<slug>` on the branch `task/<slug>`. The path is stored in
//! `tasks.workdir`, and `task_root` is where every piece of work for the task happens (roles,
//! guard, git facts, approve). A project in the default `shared` mode has no workdir on its
//! tasks, so `task_root` is the project repo, as before.

use crate::db::TaskRow;
use crate::error::{BrainError, BrainResult};
use crate::projects::Project;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Task id a child process works for (set by the dashboard job manager and the autopilot
/// driver). Only read to find the task's worktree in worktree mode.
pub const TASK_ENV: &str = "AGENT_BRAIN_TASK";

/// Folder of every task worktree: `<data_dir>/worktrees`.
pub fn worktrees_dir() -> PathBuf {
    crate::config::data_dir().join("worktrees")
}

/// Worktree of a task: `<data_dir>/worktrees/<project_id>/<slug>`.
pub fn worktree_path(project_id: &str, slug: &str) -> PathBuf {
    worktrees_dir().join(project_id).join(slug)
}

/// Where the work of `task` happens: its worktree when it has one, else the project repo.
pub fn task_root(project: &Project, task: &TaskRow) -> PathBuf {
    task_root_at(&project.root(), task)
}

/// `task_root` with the project root already resolved (`require_runnable`).
pub fn task_root_at(project_root: &Path, task: &TaskRow) -> PathBuf {
    match task.workdir.as_deref().map(str::trim) {
        Some(w) if !w.is_empty() => PathBuf::from(w),
        _ => project_root.to_path_buf(),
    }
}

/// `git worktree prune`: forgets worktrees whose folder is gone (never touches a folder that
/// exists). Errors are ignored: it only tidies git's bookkeeping.
fn prune(project_root: &Path) {
    let _ = crate::gitops::git(project_root, &["worktree", "prune"]);
}

/// Creates the worktree of `branch` (an existing branch) at the task's path and returns it.
/// An existing folder on the right branch is reused.
fn add_worktree(project_root: &Path, project_id: &str, slug: &str, branch: &str) -> BrainResult<PathBuf> {
    let path = worktree_path(project_id, slug);
    if path.is_dir() {
        if crate::gitops::current_branch(&path).as_deref() == Some(branch) {
            return Ok(path);
        }
        return Err(BrainError::error(format!(
            "Thư mục worktree {} đã có nhưng không ở nhánh '{branch}' — dọn thư mục đó trước",
            path.display()
        )));
    }
    prune(project_root);
    crate::gitops::worktree_add(project_root, &path, branch)?;
    Ok(path)
}

/// D2.2: a new task in worktree mode. Creates `branch` from `start` without touching the main
/// repo's checkout, then its worktree; returns the worktree path.
pub fn create_task_worktree(
    project_root: &Path,
    project_id: &str,
    slug: &str,
    branch: &str,
    start: &str,
) -> BrainResult<PathBuf> {
    if !crate::gitops::branch_exists(project_root, branch) {
        crate::gitops::branch_from(project_root, branch, start)?;
    }
    add_worktree(project_root, project_id, slug, branch)
}

/// Worktree mode: makes sure the task has its worktree (re-created on its existing branch after
/// a cleanup or a switch from shared mode) and returns the task root. The main repo stays the
/// root when it has the task branch checked out itself (git cannot check one branch out twice),
/// or when the branch is gone (the roles then refuse as in shared mode). Shared mode: the
/// project root.
pub fn ensure_task_root(
    conn: &Connection,
    project: &Project,
    project_root: &Path,
    task: &TaskRow,
) -> BrainResult<PathBuf> {
    if !project.workspace.is_worktree() {
        return Ok(task_root_at(project_root, task));
    }
    if let Some(w) = task.workdir.as_deref().filter(|w| !w.trim().is_empty()) {
        if Path::new(w).is_dir() {
            return Ok(PathBuf::from(w));
        }
    }
    if crate::gitops::current_branch(project_root).as_deref() == Some(task.branch.as_str())
        || !crate::gitops::branch_exists(project_root, &task.branch)
    {
        if task.workdir.is_some() {
            crate::db::set_task_workdir(conn, task.id, None)?;
        }
        return Ok(project_root.to_path_buf());
    }
    let path = add_worktree(project_root, &project.id, &task.slug, &task.branch)?;
    crate::db::set_task_workdir(conn, task.id, Some(&path.to_string_lossy()))?;
    crate::term::emit(&format!(
        "Đã tạo worktree cho task {} tại {}",
        task.id,
        path.display()
    ));
    Ok(path)
}

/// The task a role run works for, from `AGENT_BRAIN_TASK` or the autopilot task variable, when
/// it belongs to the project.
fn env_task(conn: &Connection, project_row_id: i64) -> BrainResult<Option<TaskRow>> {
    for var in [TASK_ENV, crate::autopilot::AUTOPILOT_TASK_ENV] {
        let Ok(raw) = std::env::var(var) else { continue };
        let Ok(id) = raw.trim().parse::<i64>() else { continue };
        if let Some(t) = crate::db::get_task(conn, id)? {
            if t.project_id == project_row_id {
                return Ok(Some(t));
            }
        }
    }
    Ok(None)
}

/// Root a role run (Dev, QA, reviews, inspect) works in. Shared mode: the project repo. Worktree
/// mode: the worktree of the task named by `AGENT_BRAIN_TASK` / the autopilot variable (created
/// when missing), else the task worktree the current directory is in, else the project repo (the
/// role then needs the repo on the task branch, as in shared mode).
pub fn role_root(conn: &Connection, project: &Project, project_root: &Path) -> BrainResult<PathBuf> {
    if !project.workspace.is_worktree() {
        return Ok(project_root.to_path_buf());
    }
    let Ok(pid) = crate::db::project_row_id(conn, &project.id) else {
        return Ok(project_root.to_path_buf());
    };
    if let Some(task) = env_task(conn, pid)? {
        return ensure_task_root(conn, project, project_root, &task);
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = std::fs::canonicalize(&cwd).unwrap_or(cwd);
        for t in crate::db::tasks_with_workdir(conn, pid)? {
            let w = PathBuf::from(t.workdir.as_deref().unwrap_or_default());
            let w = std::fs::canonicalize(&w).unwrap_or(w);
            if w.is_dir() && cwd.starts_with(&w) {
                return Ok(w);
            }
        }
    }
    Ok(project_root.to_path_buf())
}

/// The base a task branch is merged into: the task's own `base_branch`, else the project's.
pub fn merge_base_of(project: &Project, task: &TaskRow) -> Option<String> {
    task.base_branch
        .clone()
        .filter(|b| !b.trim().is_empty())
        .or_else(|| project.workspace.base_branch.clone())
}

/// D2.4: why the worktree of a task may not be removed (`None` = it may). Only a task that has
/// a worktree, is done (`tasks::SUCCESS_STATUSES`) or ended (autopilot `ended`), has nothing
/// running, and whose branch is merged into its base, unless the user confirmed.
pub fn cleanup_refusal(
    task: &TaskRow,
    auto_state: Option<&str>,
    busy: bool,
    merged: bool,
    confirmed: bool,
) -> Option<String> {
    if task.workdir.as_deref().is_none_or(|w| w.trim().is_empty()) {
        return Some(format!("Task {} không có worktree để dọn.", task.id));
    }
    if !crate::tasks::is_success_status(&task.status) && auto_state != Some("ended") {
        return Some(format!(
            "Chỉ dọn worktree của task đã xong ({}) hoặc đã kết thúc.",
            crate::tasks::SUCCESS_STATUSES.join(", ")
        ));
    }
    if busy {
        return Some("Task đang có run hoặc autopilot chạy — chờ xong rồi dọn.".to_string());
    }
    if !merged && !confirmed {
        return Some(format!(
            "Nhánh '{}' chưa merge vào nhánh gốc — cần xác nhận để dọn worktree (nhánh vẫn giữ).",
            task.branch
        ));
    }
    None
}

/// True when the task has a live run or a live autopilot driver.
fn task_busy(conn: &Connection, task: &TaskRow) -> BrainResult<bool> {
    if let Some(r) = crate::db::running_run_for_task(conn, task.id)? {
        if let (Some(pid), Some(start)) = (r.pid, r.pid_start) {
            if crate::procs::alive(crate::procs::ProcId {
                pid: pid as i32,
                start: start as u64,
            }) {
                return Ok(true);
            }
        }
    }
    let auto = crate::db::task_auto(conn, task.id)?.unwrap_or_default();
    Ok(crate::autopilot::claim::driver_alive(&auto))
}

/// D2.4: removes the task's worktree with `git worktree remove` (no `--force`: a dirty worktree
/// is refused) and clears `tasks.workdir`. The branch is never deleted. `Err` = the refusal.
pub fn cleanup_task(
    conn: &Connection,
    project: &Project,
    project_root: &Path,
    task: &TaskRow,
    confirmed: bool,
) -> BrainResult<Result<String, String>> {
    let auto = crate::db::task_auto(conn, task.id)?.unwrap_or_default();
    let merged = merge_base_of(project, task)
        .is_some_and(|base| crate::gitops::is_merged_into(project_root, &task.branch, &base));
    let busy = task_busy(conn, task)?;
    if let Some(r) = cleanup_refusal(task, auto.auto_state.as_deref(), busy, merged, confirmed) {
        return Ok(Err(r));
    }
    let workdir = PathBuf::from(task.workdir.as_deref().unwrap_or_default());
    if !workdir.is_dir() {
        // The folder is gone already: only git's bookkeeping and the row are left.
        prune(project_root);
        crate::db::set_task_workdir(conn, task.id, None)?;
        return Ok(Ok(format!(
            "Worktree của task {} không còn trên đĩa — đã xoá đường dẫn đã lưu.",
            task.id
        )));
    }
    let dirty = match crate::gitops::changed_paths(&workdir) {
        Ok(d) => d,
        Err(e) => return Ok(Err(e.message)),
    };
    if !dirty.is_empty() {
        let list: Vec<String> = dirty.into_iter().collect();
        return Ok(Err(format!(
            "Worktree {} có thay đổi chưa commit — không dọn:\n{}",
            workdir.display(),
            list.join("\n")
        )));
    }
    if let Err(e) = crate::gitops::worktree_remove(project_root, &workdir) {
        return Ok(Err(format!("git worktree remove từ chối: {}", e.message)));
    }
    crate::db::set_task_workdir(conn, task.id, None)?;
    Ok(Ok(format!(
        "Đã dọn worktree của task {} ({}); nhánh '{}' vẫn giữ.",
        task.id,
        workdir.display(),
        task.branch
    )))
}

/// D2.5: why a task may not be merged (`None` = it may): QA must have passed, the main repo must
/// be clean and on the base branch.
pub fn merge_refusal(
    task: &TaskRow,
    base: Option<&str>,
    current: Option<&str>,
    dirty: &[String],
) -> Option<String> {
    if task.status != "qa_pass" {
        return Some(format!(
            "Chỉ merge task đã qua QA (trạng thái hiện tại: {}).",
            task.status
        ));
    }
    let Some(base) = base else {
        return Some("Task không có nhánh gốc (base_branch) để merge vào.".to_string());
    };
    if current != Some(base) {
        return Some(format!(
            "Repo chính không ở nhánh gốc '{base}' (nhánh hiện tại: '{}') — checkout '{base}' rồi merge.",
            current.unwrap_or("None")
        ));
    }
    if !dirty.is_empty() {
        return Some(format!(
            "Repo chính có thay đổi chưa commit — không merge:\n{}",
            dirty.join("\n")
        ));
    }
    None
}

/// D2.5: `git merge --no-ff task/<slug>` into the base branch in the main repo. A conflict on a
/// protocol file keeps the base branch's version (or drops the file when the project has no
/// `protocol_dir`); any other conflict aborts the merge. Never pushes. `Err` = the refusal.
pub fn merge_task(
    project: &Project,
    project_root: &Path,
    task: &TaskRow,
) -> BrainResult<Result<String, String>> {
    let base = merge_base_of(project, task);
    let current = crate::gitops::current_branch(project_root);
    let dirty: Vec<String> = crate::gitops::changed_paths(project_root)?.into_iter().collect();
    if let Some(r) = merge_refusal(task, base.as_deref(), current.as_deref(), &dirty) {
        return Ok(Err(r));
    }
    let base = base.unwrap_or_default();
    if !crate::gitops::branch_exists(project_root, &task.branch) {
        return Ok(Err(format!("Không có nhánh '{}' để merge.", task.branch)));
    }
    let message = format!("Merge {} vào {base}", task.branch);
    if crate::gitops::git(
        project_root,
        &["merge", "--no-ff", "--no-edit", "-m", &message, &task.branch],
    )
    .is_ok()
    {
        return Ok(Ok(format!("Đã merge '{}' vào '{base}' (chưa push).", task.branch)));
    }
    let conflicts: Vec<String> = crate::gitops::git(
        project_root,
        &["diff", "--name-only", "--diff-filter=U", "-z"],
    )
    .unwrap_or_default()
    .split('\0')
    .filter(|s| !s.is_empty())
    .map(str::to_string)
    .collect();
    let protocol = crate::protocol_paths::paths(project, &task.slug);
    let (proto, other): (Vec<String>, Vec<String>) =
        conflicts.into_iter().partition(|f| protocol.contains(f));
    if conflicts_unresolvable(&proto, &other) {
        let _ = crate::gitops::git(project_root, &["merge", "--abort"]);
        let list = if other.is_empty() {
            "(không đọc được danh sách xung đột)".to_string()
        } else {
            other.join("\n")
        };
        return Ok(Err(format!(
            "Merge '{}' vào '{base}' có xung đột — đã huỷ merge (git merge --abort):\n{list}",
            task.branch
        )));
    }
    let keep_base = !project.protocol_dir.trim().is_empty();
    for f in &proto {
        let res = if keep_base {
            crate::gitops::git(project_root, &["checkout", "--ours", "--", f])
                .and_then(|_| crate::gitops::git(project_root, &["add", "--", f]))
        } else {
            crate::gitops::git(project_root, &["rm", "-q", "-f", "--", f])
        };
        if let Err(e) = res {
            let _ = crate::gitops::git(project_root, &["merge", "--abort"]);
            return Ok(Err(format!(
                "Không xử lý được xung đột file giao thức {f}: {} — đã huỷ merge",
                e.message
            )));
        }
    }
    if let Err(e) = crate::gitops::git(project_root, &["commit", "--no-edit", "-m", &message]) {
        let _ = crate::gitops::git(project_root, &["merge", "--abort"]);
        return Ok(Err(format!("Không commit được merge: {} — đã huỷ merge", e.message)));
    }
    Ok(Ok(format!(
        "Đã merge '{}' vào '{base}' (chưa push); file giao thức xung đột {}: {}",
        task.branch,
        if keep_base { "giữ bản của nhánh gốc" } else { "đã bỏ" },
        proto.join(", ")
    )))
}

/// A merge stops when a non-protocol file conflicts, or when it failed without any conflict to
/// resolve (some other git error).
fn conflicts_unresolvable(proto: &[String], other: &[String]) -> bool {
    !other.is_empty() || proto.is_empty()
}

fn task_and_project(conn: &Connection, projects: &[Project], task_id: i64) -> BrainResult<(TaskRow, Project)> {
    let task = crate::db::get_task(conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy task {task_id}")))?;
    let (slug, _) = crate::autopilot::claim::task_project(conn, task_id)?
        .ok_or_else(|| BrainError::error(format!("Không tìm thấy dự án của task {task_id}")))?;
    let project = projects
        .iter()
        .find(|p| p.id == slug)
        .cloned()
        .ok_or_else(|| BrainError::error(format!("Dự án '{slug}' không còn trong projects_map.json")))?;
    Ok((task, project))
}

/// `agent-brain task cleanup [<id>|--merged] [--yes]`.
pub fn cmd_cleanup(
    ctx: &mut crate::app::Ctx,
    task_id: Option<i64>,
    merged_only: bool,
    yes: bool,
) -> BrainResult<i32> {
    match (task_id, merged_only) {
        (Some(_), true) | (None, false) => Err(BrainError::error(
            "task cleanup cần <ID task> hoặc --merged (không dùng cả hai)",
        )),
        (Some(id), false) => {
            let (task, project) = task_and_project(&ctx.conn, &ctx.projects, id)?;
            let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
            match cleanup_task(&ctx.conn, &project, &root, &task, yes)? {
                Ok(msg) => {
                    crate::term::emit_out(&msg);
                    Ok(crate::error::EXIT_OK)
                }
                Err(reason) => Err(BrainError::error(reason)),
            }
        }
        (None, true) => {
            let mut removed = 0;
            let projects = ctx.projects.clone();
            for project in &projects {
                let Ok(pid) = crate::db::project_row_id(&ctx.conn, &project.id) else { continue };
                let tasks = crate::db::tasks_with_workdir(&ctx.conn, pid)?;
                if tasks.is_empty() {
                    continue;
                }
                let Ok(root) = crate::projects::require_runnable(project, &ctx.cfg) else { continue };
                for task in tasks {
                    // `--merged` never removes an unmerged branch's worktree.
                    match cleanup_task(&ctx.conn, project, &root, &task, false)? {
                        Ok(msg) => {
                            removed += 1;
                            crate::term::emit_out(&msg);
                        }
                        Err(reason) => crate::term::emit(&format!("Bỏ qua task {}: {reason}", task.id)),
                    }
                }
            }
            crate::term::emit_out(&format!("Đã dọn {removed} worktree."));
            Ok(crate::error::EXIT_OK)
        }
    }
}

/// `agent-brain task merge <id>` (D2.5).
pub fn cmd_merge(ctx: &mut crate::app::Ctx, task_id: i64) -> BrainResult<i32> {
    let (task, project) = task_and_project(&ctx.conn, &ctx.projects, task_id)?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
    match merge_task(&project, &root, &task)? {
        Ok(msg) => {
            crate::term::emit_out(&msg);
            Ok(crate::error::EXIT_OK)
        }
        Err(reason) => Err(BrainError::error(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: &str, workdir: Option<&str>) -> TaskRow {
        TaskRow {
            id: 7,
            project_id: 1,
            slug: "x".into(),
            title: "x".into(),
            branch: "task/x".into(),
            base_branch: Some("main".into()),
            base_commit: None,
            status: status.into(),
            created_at: 0,
            updated_at: 0,
            level: "vua".into(),
            workdir: workdir.map(str::to_string),
        }
    }

    #[test]
    fn task_root_prefers_workdir() {
        let root = Path::new("/repo");
        assert_eq!(task_root_at(root, &task("open", None)), PathBuf::from("/repo"));
        assert_eq!(task_root_at(root, &task("open", Some(" "))), PathBuf::from("/repo"));
        assert_eq!(task_root_at(root, &task("open", Some("/wt/x"))), PathBuf::from("/wt/x"));
    }

    #[test]
    fn cleanup_rules() {
        let done = task("qa_pass", Some("/wt/x"));
        assert!(cleanup_refusal(&task("qa_pass", None), None, false, true, true).is_some());
        assert!(cleanup_refusal(&task("planned", Some("/wt/x")), None, false, true, true).is_some());
        assert_eq!(cleanup_refusal(&task("planned", Some("/wt/x")), Some("ended"), false, true, false), None);
        assert!(cleanup_refusal(&done, None, true, true, true).is_some());
        assert!(cleanup_refusal(&done, None, false, false, false).is_some());
        assert_eq!(cleanup_refusal(&done, None, false, false, true), None);
        assert_eq!(cleanup_refusal(&done, None, false, true, false), None);
    }

    #[test]
    fn merge_rules() {
        let none: Vec<String> = vec![];
        let ok = task("qa_pass", None);
        assert!(merge_refusal(&task("dev_done", None), Some("main"), Some("main"), &none).is_some());
        assert!(merge_refusal(&ok, None, Some("main"), &none).is_some());
        assert!(merge_refusal(&ok, Some("main"), Some("task/x"), &none).is_some());
        assert!(merge_refusal(&ok, Some("main"), Some("main"), &["a".to_string()]).is_some());
        assert_eq!(merge_refusal(&ok, Some("main"), Some("main"), &none), None);
        assert!(conflicts_unresolvable(&[], &[]));
        assert!(conflicts_unresolvable(&[".plan.md".into()], &["a".into()]));
        assert!(!conflicts_unresolvable(&[".plan.md".into()], &[]));
    }
}
