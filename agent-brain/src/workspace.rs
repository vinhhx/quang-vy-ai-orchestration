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

/// Name of the dependency folder a node project's task worktree is linked to.
const NODE_MODULES: &str = "node_modules";

/// The exclude file git reads for every worktree of the repo (`<common dir>/info/exclude`),
/// asked of git from inside `worktree`.
fn info_exclude_path(worktree: &Path) -> Option<PathBuf> {
    let out = crate::gitops::git(worktree, &["rev-parse", "--git-path", "info/exclude"]).ok()?;
    let p = PathBuf::from(out.trim());
    if p.as_os_str().is_empty() {
        return None;
    }
    Some(if p.is_absolute() { p } else { worktree.join(p) })
}

/// Appends patterns to the repo's `info/exclude` when not already present. Only appends;
/// other lines stay as they are.
fn ensure_patterns_excluded(worktree: &Path, patterns: &[&str]) -> std::io::Result<()> {
    let Some(path) = info_exclude_path(worktree) else {
        return Err(std::io::Error::other("git không cho biết đường dẫn info/exclude"));
    };
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let existing_lines: Vec<&str> = existing.lines().map(str::trim).collect();
    let mut to_add = Vec::new();
    for &p in patterns {
        let p_trimmed = p.trim().trim_matches('/');
        let p_slash = format!("{p_trimmed}/");
        let p_lead_slash = format!("/{p_trimmed}");
        let p_both_slash = format!("/{p_trimmed}/");
        if !existing_lines.iter().any(|&l| {
            l == p_trimmed || l == p_slash.as_str() || l == p_lead_slash.as_str() || l == p_both_slash.as_str()
        }) {
            to_add.push(p);
        }
    }
    if to_add.is_empty() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut add = String::new();
    if !existing.is_empty() && !existing.ends_with('\n') {
        add.push('\n');
    }
    for p in to_add {
        add.push_str(p);
        add.push('\n');
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    f.write_all(add.as_bytes())
}

/// Appends the line `node_modules` (no slash, so it also matches a symlink) to the repo's
/// `info/exclude` when no such line is there yet. Only appends; other lines stay as they are.
fn ensure_node_modules_excluded(worktree: &Path) -> std::io::Result<()> {
    ensure_patterns_excluded(worktree, &[NODE_MODULES])
}

/// A node project's task worktree has no `node_modules` (the folder is ignored, so a checkout
/// never has it). When `project_root` has a `package.json` and a real `node_modules` folder and
/// the worktree has no `node_modules` of its own (not even a dangling link), links the worktree's
/// to the project's (absolute target) and makes git ignore the link (`info/exclude`), so the
/// worktree stays clean for the Lead's dirty check. Safe to call again. Problems are warnings:
/// they never fail the creation of the worktree.
pub fn link_node_modules(project_root: &Path, worktree: &Path) {
    let source = project_root.join(NODE_MODULES);
    let source_is_real_dir = std::fs::symlink_metadata(&source).is_ok_and(|m| m.file_type().is_dir());
    if !project_root.join("package.json").is_file() || !source_is_real_dir {
        return;
    }
    let link = worktree.join(NODE_MODULES);
    match std::fs::symlink_metadata(&link) {
        Ok(m) if !m.file_type().is_symlink() => return, // the worktree has its own
        Ok(_) => {}                                      // already linked
        Err(_) => {
            let target = match std::fs::canonicalize(&source) {
                Ok(t) => t,
                Err(e) => {
                    crate::term::emit(&format!("Cảnh báo: không nối được node_modules: {e}"));
                    return;
                }
            };
            if let Err(e) = std::os::unix::fs::symlink(&target, &link) {
                crate::term::emit(&format!(
                    "Cảnh báo: không nối được node_modules vào {}: {e}",
                    worktree.display()
                ));
                return;
            }
        }
    }
    if let Err(e) = ensure_node_modules_excluded(worktree) {
        crate::term::emit(&format!(
            "Cảnh báo: không ghi được node_modules vào info/exclude: {e}"
        ));
    }
}

/// Copies `vendor` into `worktree` for PHP projects. Symlinks inside (e.g. `vendor/modules/x -> ../../app-modules/x/`)
/// are preserved as relative links, resolving within the worktree. Safe to call again.
/// Problems are warnings: they never fail the creation of the worktree.
pub fn copy_vendor(project_root: &Path, worktree: &Path) {
    if !project_root.join("composer.json").is_file() {
        return;
    }
    let source = project_root.join("vendor");
    let source_is_real_dir = std::fs::symlink_metadata(&source).is_ok_and(|m| m.file_type().is_dir());
    if !source_is_real_dir {
        return;
    }
    let dest = worktree.join("vendor");
    if let Ok(m) = std::fs::symlink_metadata(&dest) {
        if m.file_type().is_symlink() {
            crate::term::emit(&format!(
                "Cảnh báo: worktree {} có vendor là symlink (làm autoload trỏ về repo chính), đang xoá symlink để copy thật",
                worktree.display()
            ));
            if let Err(e) = std::fs::remove_file(&dest) {
                crate::term::emit(&format!("Cảnh báo: không xoá được symlink vendor cũ: {e}"));
                return;
            }
        } else if m.file_type().is_dir() {
            let _ = ensure_patterns_excluded(worktree, &["vendor/", ".vendor.agent-brain-tmp/"]);
            return;
        }
    }
    let tmp = worktree.join(".vendor.agent-brain-tmp");
    if std::fs::symlink_metadata(&tmp).is_ok() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    let res = std::process::Command::new("cp")
        .arg("-a")
        .arg("--reflink=auto")
        .arg(&source)
        .arg(&tmp)
        .output();
    match res {
        Ok(out) if out.status.success() => {
            if let Err(e) = std::fs::rename(&tmp, &dest) {
                crate::term::emit(&format!(
                    "Cảnh báo: không đổi tên .vendor.agent-brain-tmp sang vendor: {e}"
                ));
                let _ = std::fs::remove_dir_all(&tmp);
                return;
            }
            let mb_str = match std::process::Command::new("du").arg("-sm").arg(&dest).output() {
                Ok(du_out) if du_out.status.success() => {
                    let stdout = String::from_utf8_lossy(&du_out.stdout);
                    if let Some(n) = stdout.split_whitespace().next() {
                        format!(" ({n} MB)")
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            };
            crate::term::emit(&format!("Đã copy vendor vào worktree{mb_str}"));
        }
        Ok(out) => {
            let _ = std::fs::remove_dir_all(&tmp);
            let err = String::from_utf8_lossy(&out.stderr);
            let err_short = err.trim().lines().next().unwrap_or("cp thất bại");
            crate::term::emit(&format!("Cảnh báo: copy vendor thất bại: {err_short}"));
            return;
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            crate::term::emit(&format!("Cảnh báo: không chạy được lệnh cp vendor: {e}"));
            return;
        }
    }
    if let Err(e) = ensure_patterns_excluded(worktree, &["vendor/", ".vendor.agent-brain-tmp/"]) {
        crate::term::emit(&format!(
            "Cảnh báo: không ghi được vendor vào info/exclude: {e}"
        ));
    }
}

/// Creates the worktree of `branch` (an existing branch) at the task's path and returns it.
/// An existing folder on the right branch is reused. `link`: link the project's `node_modules`
/// into it (`link_node_modules`, the project's `link_node_modules` option). `copy_vendor_opt`: copy
/// the project's `vendor` folder into it.
fn add_worktree(
    project_root: &Path,
    project_id: &str,
    slug: &str,
    branch: &str,
    link: bool,
    copy_vendor_opt: bool,
) -> BrainResult<PathBuf> {
    let path = worktree_path(project_id, slug);
    if path.is_dir() {
        if crate::gitops::current_branch(&path).as_deref() == Some(branch) {
            if link {
                link_node_modules(project_root, &path);
            }
            if copy_vendor_opt {
                copy_vendor(project_root, &path);
            }
            return Ok(path);
        }
        return Err(BrainError::error(format!(
            "Thư mục worktree {} đã có nhưng không ở nhánh '{branch}' — dọn thư mục đó trước",
            path.display()
        )));
    }
    prune(project_root);
    crate::gitops::worktree_add(project_root, &path, branch)?;
    if link {
        link_node_modules(project_root, &path);
    }
    if copy_vendor_opt {
        copy_vendor(project_root, &path);
    }
    Ok(path)
}

/// D2.2: a new task in worktree mode. Creates `branch` from `start` without touching the main
/// repo's checkout, then its worktree; returns the worktree path. `link`: the project's
/// `link_node_modules` option. Defaults `copy_vendor` to true.
pub fn create_task_worktree(
    project_root: &Path,
    project_id: &str,
    slug: &str,
    branch: &str,
    start: &str,
    link: bool,
) -> BrainResult<PathBuf> {
    create_task_worktree_opts(project_root, project_id, slug, branch, start, link, true)
}

/// `create_task_worktree` with explicit options for `link_node_modules` and `copy_vendor`.
pub fn create_task_worktree_opts(
    project_root: &Path,
    project_id: &str,
    slug: &str,
    branch: &str,
    start: &str,
    link: bool,
    copy_vendor_opt: bool,
) -> BrainResult<PathBuf> {
    if !crate::gitops::branch_exists(project_root, branch) {
        crate::gitops::branch_from(project_root, branch, start)?;
    }
    add_worktree(project_root, project_id, slug, branch, link, copy_vendor_opt)
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
            if project.workspace.link_node_modules {
                link_node_modules(project_root, Path::new(w));
            }
            if project.workspace.copy_vendor {
                copy_vendor(project_root, Path::new(w));
            }
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
    let path = add_worktree(
        project_root,
        &project.id,
        &task.slug,
        &task.branch,
        project.workspace.link_node_modules,
        project.workspace.copy_vendor,
    )?;
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
pub(crate) fn task_busy(conn: &Connection, task: &TaskRow) -> BrainResult<bool> {
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
    let (proto, other) = merge_conflicts(project, project_root, task);
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
    let keep_base = match resolve_protocol_conflicts(project, project_root, &proto) {
        Ok(k) => k,
        Err(msg) => {
            let _ = crate::gitops::git(project_root, &["merge", "--abort"]);
            return Ok(Err(format!("{msg} — đã huỷ merge")));
        }
    };
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

/// The conflicted paths of a merge in progress, split into the task's protocol files and the
/// rest (shared by `merge_task` and `merge_push_task`).
fn merge_conflicts(project: &Project, root: &Path, task: &TaskRow) -> (Vec<String>, Vec<String>) {
    let conflicts: Vec<String> =
        crate::gitops::git(root, &["diff", "--name-only", "--diff-filter=U", "-z"])
            .unwrap_or_default()
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    let protocol = crate::protocol_paths::paths(project, &task.slug);
    conflicts.into_iter().partition(|f| protocol.contains(f))
}

/// Resolves conflicted protocol files of a merge in progress: keeps the base branch's version
/// when the project has a `protocol_dir`, else drops the file. Returns whether the base version
/// was kept. `Err` = the reason; the caller aborts the merge.
fn resolve_protocol_conflicts(
    project: &Project,
    root: &Path,
    proto: &[String],
) -> Result<bool, String> {
    let keep_base = !project.protocol_dir.trim().is_empty();
    for f in proto {
        let res = if keep_base {
            crate::gitops::git(root, &["checkout", "--ours", "--", f])
                .and_then(|_| crate::gitops::git(root, &["add", "--", f]))
        } else {
            crate::gitops::git(root, &["rm", "-q", "-f", "--", f])
        };
        if let Err(e) = res {
            return Err(format!(
                "Không xử lý được xung đột file giao thức {f}: {}",
                e.message
            ));
        }
    }
    Ok(keep_base)
}

// ---------------------------------------------------------------------------------------------
// Merge & push (task merge --push, dashboard control `merge_push`, Telegram button)
// ---------------------------------------------------------------------------------------------

/// Where a task branch stands against its base, read from local refs only (no network).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeState {
    /// A: the task branch is not in the local base branch yet.
    NotMerged,
    /// B: merged into the local base branch, but `origin/<base>` does not have it.
    MergedLocal,
    /// C: `origin/<base>` (as last fetched) contains the task branch.
    Pushed,
    /// The task branch is gone and no merge of the task is found.
    NoBranch,
}

/// `[task #<id>]`, the mark every merge commit of `merge_push_task` carries in its subject.
pub fn task_mark(task_id: i64) -> String {
    format!("[task #{task_id}]")
}

/// True when a first-parent commit of `rev` has `mark` in its message.
fn first_parent_has(root: &Path, rev: &str, mark: &str) -> bool {
    let grep = format!("--grep={mark}");
    crate::gitops::git(
        root,
        &[
            "log",
            "--first-parent",
            "--fixed-strings",
            &grep,
            "--format=%H",
            "--max-count=1",
            rev,
            "--",
        ],
    )
    .map(|o| !o.trim().is_empty())
    .unwrap_or(false)
}

/// Reads `MergeState` of `branch` against `base` from local refs (`refs/heads/<base>` and
/// `refs/remotes/origin/<base>`). Both names must have passed `gitops::valid_branch_name`.
pub fn merge_state(root: &Path, task_id: i64, branch: &str, base: &str) -> MergeState {
    let remote = format!("refs/remotes/origin/{base}");
    let local = format!("refs/heads/{base}");
    let remote_sha = crate::gitops::rev_parse(root, &remote);
    let local_sha = crate::gitops::rev_parse(root, &local);
    match crate::gitops::rev_parse(root, &format!("refs/heads/{branch}")) {
        Some(tip) => {
            if remote_sha
                .as_deref()
                .is_some_and(|r| crate::gitops::is_ancestor(root, &tip, r))
            {
                MergeState::Pushed
            } else if local_sha
                .as_deref()
                .is_some_and(|l| crate::gitops::is_ancestor(root, &tip, l))
            {
                MergeState::MergedLocal
            } else {
                MergeState::NotMerged
            }
        }
        None => {
            let mark = task_mark(task_id);
            if remote_sha.is_some() && first_parent_has(root, &remote, &mark) {
                MergeState::Pushed
            } else if local_sha.is_some() && first_parent_has(root, &local, &mark) {
                MergeState::MergedLocal
            } else {
                MergeState::NoBranch
            }
        }
    }
}

/// The base branch of Merge & push: the task's, else the project's, else
/// `merge.default_base_branch`.
pub fn merge_push_base(cfg: &crate::config::Config, project: &Project, task: &TaskRow) -> String {
    merge_base_of(project, task).unwrap_or_else(|| cfg.merge.default_base_branch.clone())
}

/// Result of `merge_push_task`. Nothing of it is stored: the next call reads the state from git.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePushOutcome {
    pub base: String,
    pub branch: String,
    /// The merge commit made by this call.
    pub merge_commit: Option<String>,
    /// `origin/<base>` has the task branch now (pushed by this call or before).
    pub pushed: bool,
    pub branch_deleted: bool,
    /// Nothing was left to merge or push.
    pub already_done: bool,
    /// The push failed; the local merge is kept and pressing again only pushes.
    pub push_failed: bool,
    pub notes: Vec<String>,
}

impl MergePushOutcome {
    /// A short Vietnamese summary for the terminal, the dashboard and Telegram.
    pub fn message(&self) -> String {
        let (b, base) = (&self.branch, &self.base);
        let sha7 = self
            .merge_commit
            .as_deref()
            .map(|s| s.chars().take(7).collect::<String>());
        let head = match (self.already_done, self.pushed, sha7) {
            (true, true, _) => format!("Nhánh '{b}' đã merge vào '{base}' và push từ trước."),
            (true, false, _) => format!("Nhánh '{b}' đã merge vào '{base}' từ trước."),
            (false, true, Some(s)) => {
                format!("Đã merge '{b}' vào '{base}' ({s}) và push lên origin.")
            }
            (false, true, None) => {
                format!("Đã push '{base}' lên origin (merge '{b}' đã có từ lần trước).")
            }
            (false, false, Some(s)) => format!("Đã merge '{b}' vào '{base}' ({s}), chưa push."),
            (false, false, None) => format!("Nhánh '{b}' đã merge vào '{base}', chưa push."),
        };
        let mut lines = vec![head];
        if self.branch_deleted {
            lines.push(format!("Đã xoá nhánh cục bộ '{b}'."));
        }
        lines.extend(self.notes.iter().cloned());
        lines.join("\n")
    }
}

/// One Merge & push at a time in this process (two presses at once never run git side by side).
static MERGE_PUSH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Most paths a refusal lists.
const MAX_LISTED: usize = 10;

fn listed(paths: &[String]) -> String {
    let mut lines: Vec<String> = paths.iter().take(MAX_LISTED).cloned().collect();
    if paths.len() > MAX_LISTED {
        lines.push(format!("… và {} file khác", paths.len() - MAX_LISTED));
    }
    lines.join("\n")
}

/// Text from git made safe to show: the remote URL becomes `origin`, the repo path `<repo>`, any
/// `user:pass@` in a URL is masked; at most the last 3 lines and 300 characters.
fn scrub(text: &str, root: &Path, url: Option<&str>) -> String {
    let mut s = text.to_string();
    if let Some(u) = url.filter(|u| !u.trim().is_empty()) {
        s = s.replace(u, "origin");
    }
    let r = root.display().to_string();
    if r.len() > 1 {
        s = s.replace(&r, "<repo>");
    }
    if let Ok(re) = regex::Regex::new(r"([A-Za-z][A-Za-z0-9+.-]*://)[^/@\s]+@") {
        s = re.replace_all(&s, "${1}***@").into_owned();
    }
    let lines: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" · ");
    if tail.chars().count() > 300 {
        let cut: String = tail.chars().take(299).collect();
        format!("{cut}…")
    } else {
        tail
    }
}

/// A name shown in a refusal: control characters removed, at most 80 characters.
fn shown_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(80).collect()
}

/// The merge commit message: `Merge <branch> vào <base> [task #<id>]: <title>` and two lines of
/// facts. The title is its first line without control characters, at most 80 characters.
pub fn merge_push_message(project: &Project, task: &TaskRow, base: &str) -> String {
    let title: String = task
        .title
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(80)
        .collect::<String>()
        .trim()
        .to_string();
    let head = format!(
        "Merge {} vào {base} {}",
        task.branch,
        task_mark(task.id)
    );
    let subject = if title.is_empty() {
        head
    } else {
        format!("{head}: {title}")
    };
    format!(
        "{subject}\n\nDự án: {}\nTask: #{} · QA: đạt",
        project.id, task.id
    )
}

/// The merge step of Merge & push on the base branch (checked out): `git merge --no-ff
/// --no-commit`, conflicts on protocol files resolved as `merge_task` does, any other conflict
/// aborts. Protocol files the base branch does not track are taken out of the result (the task
/// branch keeps them in its history, the merge's second parent); tracked ones keep the base
/// version. Returns the merge commit; `Err` = the reason (the merge was aborted).
fn merge_for_push(
    project: &Project,
    root: &Path,
    task: &TaskRow,
    base: &str,
    url: Option<&str>,
) -> Result<String, String> {
    let id = task.id;
    let message = merge_push_message(project, task, base);
    let task_ref = format!("refs/heads/{}", task.branch);
    crate::term::emit(&format!("task {id}: merge {} vào {base}", task.branch));
    let abort = || {
        let _ = crate::gitops::git(root, &["merge", "--abort"]);
    };
    if let Err(e) = crate::gitops::git(root, &["merge", "--no-ff", "--no-commit", &task_ref]) {
        let (proto, other) = merge_conflicts(project, root, task);
        if conflicts_unresolvable(&proto, &other) {
            abort();
            let list = if other.is_empty() {
                scrub(&e.message, root, url)
            } else {
                listed(&other)
            };
            return Err(format!(
                "Merge '{}' vào '{base}' có xung đột — đã huỷ merge (git merge --abort):\n{list}",
                task.branch
            ));
        }
        match resolve_protocol_conflicts(project, root, &proto) {
            Ok(keep_base) => crate::term::emit(&format!(
                "task {id}: xung đột file giao thức {}: {}",
                if keep_base { "giữ bản của nhánh gốc" } else { "đã bỏ" },
                proto.join(", ")
            )),
            Err(msg) => {
                abort();
                return Err(format!("{msg} — đã huỷ merge"));
            }
        }
    }
    let protocol = crate::protocol_paths::paths(project, &task.slug);
    let mut stripped: Vec<String> = Vec::new();
    for p in protocol.all() {
        let on_base = crate::gitops::git(root, &["ls-tree", "--name-only", "HEAD", "--", p])
            .map(|o| !o.trim().is_empty())
            .unwrap_or(false);
        let res = if on_base {
            let changed = crate::gitops::git(root, &["diff", "--cached", "--name-only", "HEAD", "--", p])
                .map(|o| !o.trim().is_empty())
                .unwrap_or(false);
            if !changed {
                continue;
            }
            crate::gitops::git(root, &["checkout", "HEAD", "--", p])
        } else {
            let in_index = crate::gitops::git(root, &["ls-files", "--cached", "--", p])
                .map(|o| !o.trim().is_empty())
                .unwrap_or(false);
            if !in_index {
                continue;
            }
            let r = crate::gitops::git(root, &["rm", "-q", "--cached", "--", p]);
            if r.is_ok() {
                let _ = std::fs::remove_file(root.join(p));
            }
            r
        };
        if let Err(e) = res {
            abort();
            return Err(format!(
                "Không loại được file giao thức {p} khỏi merge: {} — đã huỷ merge",
                scrub(&e.message, root, url)
            ));
        }
        stripped.push(p.to_string());
    }
    if !stripped.is_empty() {
        crate::term::emit(&format!(
            "task {id}: loại file giao thức khỏi merge: {}",
            stripped.join(", ")
        ));
    }
    if let Err(e) = crate::gitops::git(root, &["commit", "-q", "-m", &message]) {
        abort();
        return Err(format!(
            "Không commit được merge: {} — đã huỷ merge",
            scrub(&e.message, root, url)
        ));
    }
    crate::gitops::head_sha(root).map_err(|e| e.message)
}

/// `git branch -d` of the task branch after it reached origin (never `-D`). Not done for a task
/// with a worktree; a failure is a note, never a failure of the whole action.
fn delete_task_branch(
    cfg: &crate::config::Config,
    root: &Path,
    task: &TaskRow,
    url: Option<&str>,
    out: &mut MergePushOutcome,
) {
    if !cfg.merge.delete_task_branch || !crate::gitops::branch_exists(root, &task.branch) {
        return;
    }
    if task.workdir.as_deref().is_some_and(|w| !w.trim().is_empty()) {
        out.notes.push(format!(
            "Task có worktree — bấm Dọn worktree trước; nhánh '{}' vẫn giữ.",
            task.branch
        ));
        return;
    }
    match crate::gitops::git(root, &["branch", "-d", "--", &task.branch]) {
        Ok(_) => {
            out.branch_deleted = true;
            crate::term::emit(&format!("task {}: đã xoá nhánh {}", task.id, task.branch));
        }
        Err(e) => {
            let msg = format!(
                "Không xoá được nhánh '{}': {}",
                task.branch,
                scrub(&e.message, root, url)
            );
            crate::term::emit(&format!("task {}: {msg}", task.id));
            out.notes.push(msg);
        }
    }
}

/// Merge & push of a QA-passed task (the CLI `task merge --push`, the dashboard control
/// `merge_push` and the Telegram button). Fetches `origin/<base>`, checks out the base branch
/// (created from origin when missing, fast-forwarded when behind), merges the task branch with
/// `--no-ff` (protocol files left out), pushes `<base>` without force and deletes the local task
/// branch with `git branch -d`. Safe to run again after any failure: the state is read from git
/// (`MergeState`), so a failed push leaves the merge in place and the next call only pushes.
/// `busy` is the project's busy reason. `Ok(Err)` = the refusal (nothing changed in the repo);
/// `Err` = a system error. Never `--force`, `reset --hard`, `clean`, `branch -D`, or a shell.
pub fn merge_push_task(
    cfg: &crate::config::Config,
    project: &Project,
    root: &Path,
    task: &TaskRow,
    busy: Option<String>,
) -> BrainResult<Result<MergePushOutcome, String>> {
    let _one = MERGE_PUSH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let id = task.id;
    let base = merge_push_base(cfg, project, task);
    let branch = task.branch.clone();
    let refuse = |reason: String| -> BrainResult<Result<MergePushOutcome, String>> {
        crate::term::emit(&format!(
            "task {id}: Merge & push từ chối — {}",
            reason.lines().next().unwrap_or("")
        ));
        Ok(Err(reason))
    };

    // Names first: nothing below runs git with a name that did not pass the check.
    if !crate::gitops::valid_branch_name(&base) {
        return refuse(format!("Tên nhánh gốc không hợp lệ: '{}'", shown_name(&base)));
    }
    if !crate::gitops::valid_branch_name(&branch) {
        return refuse(format!("Tên nhánh task không hợp lệ: '{}'", shown_name(&branch)));
    }

    let state0 = merge_state(root, id, &branch, &base);
    if task.status != "qa_pass" && state0 != MergeState::Pushed {
        let reason = merge_refusal(task, Some(&base), Some(&base), &[])
            .unwrap_or_else(|| "Chỉ merge task đã qua QA.".to_string());
        return refuse(reason);
    }
    if let Some(b) = busy {
        return refuse(b);
    }
    let dirty: Vec<String> = match crate::gitops::changed_paths(root) {
        Ok(d) => d.into_iter().collect(),
        Err(e) => return refuse(format!("Không đọc được trạng thái repo: {}", e.message)),
    };
    if !dirty.is_empty() {
        let protocol = crate::protocol_paths::paths(project, &task.slug);
        let reason = if dirty.iter().all(|p| protocol.contains(p)) {
            format!(
                "Repo chính chỉ còn file giao thức chưa commit — commit hoặc bỏ chúng rồi merge:\n{}",
                listed(&dirty)
            )
        } else {
            format!(
                "Repo chính có thay đổi chưa commit — không merge:\n{}",
                listed(&dirty)
            )
        };
        return refuse(reason);
    }
    let has_branch = crate::gitops::branch_exists(root, &branch);
    if !has_branch && state0 != MergeState::Pushed {
        return refuse(format!("Không có nhánh '{branch}' để merge."));
    }

    let url = crate::gitops::remote_url(root, "origin");
    let mut out = MergePushOutcome {
        base: base.clone(),
        branch: branch.clone(),
        ..MergePushOutcome::default()
    };
    let orig = crate::gitops::current_branch(root);
    let orig_sha = crate::gitops::head_sha(root).ok();
    let on_base = orig.as_deref() == Some(base.as_str());
    let back = || {
        if on_base {
            return;
        }
        let res = match (orig.as_deref(), orig_sha.as_deref()) {
            (Some(b), _) => crate::gitops::git(root, &["checkout", b, "--"]),
            (None, Some(sha)) => crate::gitops::git(root, &["checkout", "--detach", sha, "--"]),
            (None, None) => return,
        };
        if let Err(e) = res {
            crate::term::emit(&format!("task {id}: không quay lại được nhánh ban đầu: {}", e.message));
        }
    };

    let Some(url) = url else {
        // No origin: merge locally, nothing to push.
        out.notes.push("Repo không có origin, chưa push.".to_string());
        if state0 != MergeState::NotMerged {
            out.already_done = true;
            return Ok(Ok(out));
        }
        if !crate::gitops::branch_exists(root, &base) {
            return refuse(format!(
                "Không có nhánh gốc '{base}' trên repo (và repo không có origin)."
            ));
        }
        if !on_base {
            crate::term::emit(&format!("task {id}: checkout {base}"));
            if let Err(e) = crate::gitops::git(root, &["checkout", &base, "--"]) {
                return refuse(format!(
                    "Không checkout được '{base}': {}",
                    scrub(&e.message, root, None)
                ));
            }
        }
        return match merge_for_push(project, root, task, &base, None) {
            Ok(sha) => {
                out.merge_commit = Some(sha);
                Ok(Ok(out))
            }
            Err(reason) => {
                back();
                refuse(reason)
            }
        };
    };
    let url_ref = Some(url.as_str());
    let timeout = std::time::Duration::from_secs(cfg.merge.git_timeout_secs);

    // a. fetch
    crate::term::emit(&format!("task {id}: fetch origin {base}"));
    let base_ref = format!("refs/heads/{base}");
    let remote_ref = format!("refs/remotes/origin/{base}");
    // Explicit `src:dst` refspec: a bare `fetch origin <ref>` only updates the tracking ref when
    // `remote.origin.fetch` maps it, and repos without that line (non-standard clone or
    // `remote add`) would never get `origin/<base>`. `base` passed `valid_branch_name` above, so it
    // holds no `:`, `*`, `?`, `[`, `\`, space or `..` that could break the refspec. The `+` applies
    // to the local tracking ref only; the local branch is untouched.
    let refspec = format!("+{base_ref}:{remote_ref}");
    if let Err(e) = crate::gitops::git_net(root, &["fetch", "origin", &refspec], timeout) {
        return refuse(format!(
            "Không fetch được origin/{base}: {}",
            scrub(&e.message, root, url_ref)
        ));
    }
    if crate::gitops::rev_parse(root, &remote_ref).is_none() {
        return refuse(format!("Không thấy origin/{base} sau khi fetch."));
    }
    let state = merge_state(root, id, &branch, &base);
    let local_base = crate::gitops::branch_exists(root, &base);

    // b. the local base against origin/<base>
    let (ahead, behind) = if local_base {
        match crate::gitops::ahead_behind(root, &base_ref, &remote_ref) {
            Ok(ab) => ab,
            Err(e) => return refuse(format!("Không so được '{base}' với origin/{base}: {}", e.message)),
        }
    } else {
        (0, 0)
    };
    let range = format!("{remote_ref}..{base_ref}");
    let only_this_task = |n: usize| {
        let mark = task_mark(id);
        let subjects = crate::gitops::log_subjects(root, &range, n.max(1));
        !subjects.is_empty() && subjects.iter().all(|s| s.contains(&mark))
    };
    let valid_b = state == MergeState::MergedLocal && behind == 0 && ahead > 0 && only_this_task(ahead);
    if ahead > 0 && state != MergeState::Pushed && !valid_b {
        if behind > 0 {
            return refuse(format!(
                "Nhánh '{base}' cục bộ và origin/{base} đã tách nhau ({ahead} commit chưa push, {behind} commit mới trên origin) — xử lý tay rồi thử lại."
            ));
        }
        let subjects = crate::gitops::log_subjects(root, &range, 5);
        let mut text = format!(
            "Nhánh '{base}' cục bộ có {ahead} commit chưa push (push '{base}' sẽ kéo theo chúng) — push hoặc xử lý chúng trước:"
        );
        for s in subjects {
            text.push_str(&format!("\n- {}", shown_name(&s)));
        }
        return refuse(text);
    }

    // c. checkout the base branch (created from origin when missing), fast-forward when behind.
    // `Err` = the reason; the repo is back where it was.
    let checkout_base = || -> Result<(), String> {
        if !on_base {
            crate::term::emit(&format!("task {id}: checkout {base}"));
            let origin_base = format!("origin/{base}");
            let res = if local_base {
                crate::gitops::git(root, &["checkout", &base, "--"])
            } else {
                crate::gitops::git(root, &["checkout", "-b", &base, "--track", &origin_base])
            };
            if let Err(e) = res {
                return Err(format!(
                    "Không checkout được '{base}': {}",
                    scrub(&e.message, root, url_ref)
                ));
            }
        }
        if local_base && behind > 0 && ahead == 0 {
            crate::term::emit(&format!("task {id}: fast-forward {base} theo origin/{base}"));
            if let Err(e) = crate::gitops::git(root, &["merge", "--ff-only", &remote_ref]) {
                back();
                return Err(format!(
                    "Không fast-forward được '{base}' theo origin/{base}: {}",
                    scrub(&e.message, root, url_ref)
                ));
            }
        }
        Ok(())
    };

    if state == MergeState::Pushed {
        // Nothing to merge or push; only the task branch may be left to delete (`git branch -d`
        // needs it merged into HEAD, so the repo goes to the base branch first).
        out.already_done = true;
        out.pushed = true;
        crate::term::emit(&format!("task {id}: đã merge và push từ trước"));
        let wants_delete = cfg.merge.delete_task_branch
            && has_branch
            && task.workdir.as_deref().is_none_or(|w| w.trim().is_empty());
        if wants_delete {
            if let Err(reason) = checkout_base() {
                out.notes.push(format!("Nhánh '{branch}' vẫn giữ: {reason}"));
                return Ok(Ok(out));
            }
        }
        delete_task_branch(cfg, root, task, url_ref, &mut out);
        return Ok(Ok(out));
    }
    if let Err(reason) = checkout_base() {
        return refuse(reason);
    }

    // d. merge
    if state == MergeState::NotMerged {
        match merge_for_push(project, root, task, &base, url_ref) {
            Ok(sha) => out.merge_commit = Some(sha),
            Err(reason) => {
                back();
                return refuse(reason);
            }
        }
    }

    // e/f. push (never forced)
    let refspec = format!("{base_ref}:{base_ref}");
    crate::term::emit(&format!("task {id}: push origin {base}"));
    match crate::gitops::git_net(root, &["push", "origin", &refspec], timeout) {
        Ok(_) => {
            out.pushed = true;
            let sha = crate::gitops::rev_parse(root, &base_ref).unwrap_or_default();
            crate::term::emit(&format!(
                "task {id}: đã push {base} ({})",
                sha.chars().take(7).collect::<String>()
            ));
        }
        Err(e) => {
            let err = scrub(&e.message, root, url_ref);
            crate::term::emit(&format!("task {id}: push lỗi — {err}"));
            out.push_failed = true;
            out.notes.push(format!("Push lỗi: {err}"));
            out.notes
                .push("Bấm Merge & push lần nữa để thử push lại.".to_string());
            return Ok(Ok(out));
        }
    }

    // g. the task branch is on origin now
    delete_task_branch(cfg, root, task, url_ref, &mut out);
    Ok(Ok(out))
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
    branches: bool,
) -> BrainResult<i32> {
    match (task_id, merged_only) {
        (Some(_), true) | (None, false) => Err(BrainError::error(
            "task cleanup cần <ID task> hoặc --merged (không dùng cả hai)",
        )),
        (Some(_), false) if branches => Err(BrainError::error(
            "--branches chỉ dùng cùng --merged (dọn mọi nhánh task đã merge)",
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
            if branches {
                let mut deleted = 0;
                for project in &projects {
                    if !project.workspace.is_worktree() {
                        continue;
                    }
                    let Ok(pid) = crate::db::project_row_id(&ctx.conn, &project.id) else { continue };
                    if crate::db::tasks_of_project(&ctx.conn, pid)?.is_empty() {
                        continue;
                    }
                    let Ok(root) = crate::projects::require_runnable(project, &ctx.cfg) else { continue };
                    let out = crate::orchestrator::housekeeping::sweep_merged_branches(
                        &ctx.conn, project, &root,
                    )?;
                    deleted += out.deleted.len();
                    for line in &out.deleted {
                        crate::term::emit_out(line);
                    }
                    for (id, reason) in &out.refused {
                        crate::term::emit(&format!("Bỏ qua task {id}: {reason}"));
                    }
                }
                crate::term::emit_out(&format!("Đã xoá {deleted} nhánh đã merge."));
            }
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

/// Why Merge & push waits when the task itself has a live run or autopilot driver.
pub const MERGE_TASK_BUSY: &str = "Task đang có run hoặc autopilot chạy — chờ xong rồi merge.";

/// The busy reason Merge & push uses outside the dashboard: the task's own run or driver, then
/// the project's (`claim::busy_reason_ws`, the check `run` and `auto` use).
pub fn merge_busy_cli(conn: &Connection, project: &Project, task: &TaskRow) -> BrainResult<Option<String>> {
    if task_busy(conn, task)? {
        return Ok(Some(MERGE_TASK_BUSY.to_string()));
    }
    crate::autopilot::claim::busy_reason_ws(
        conn,
        &project.id,
        &project.profile,
        None,
        &project.workspace,
    )
}

/// `agent-brain task merge <id> --push`: Merge & push. Exit 0 when pushed, already done or
/// merged in a repo without origin; `EXIT_ERROR` when refused or the push failed.
pub fn cmd_merge_push(ctx: &mut crate::app::Ctx, task_id: i64) -> BrainResult<i32> {
    let (task, project) = task_and_project(&ctx.conn, &ctx.projects, task_id)?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
    let busy = merge_busy_cli(&ctx.conn, &project, &task)?;
    match merge_push_task(&ctx.cfg, &project, &root, &task, busy)? {
        Ok(outcome) => {
            crate::term::emit_out(&outcome.message());
            Ok(if outcome.push_failed {
                crate::error::EXIT_ERROR
            } else {
                crate::error::EXIT_OK
            })
        }
        Err(reason) => Err(BrainError::error(reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_push_message_shape() {
        let project = Project::from_value(&serde_json::json!({
            "id": "web", "path": "/x", "name_vi": "web", "aliases": [], "type": "code",
            "repo": null, "profile": "work", "missing": false
        }))
        .unwrap();
        let mut t = task("qa_pass", None);
        t.title = "Sửa\u{7} nút   thanh toán\ndòng hai".into();
        let msg = merge_push_message(&project, &t, "dev");
        assert_eq!(
            msg,
            "Merge task/x vào dev [task #7]: Sửa nút   thanh toán\n\nDự án: web\nTask: #7 · QA: đạt"
        );
        t.title = "  ".into();
        assert!(merge_push_message(&project, &t, "dev").starts_with("Merge task/x vào dev [task #7]\n"));
        t.title = "ă".repeat(100);
        let first = merge_push_message(&project, &t, "dev");
        let subject = first.lines().next().unwrap();
        assert_eq!(subject, format!("Merge task/x vào dev [task #7]: {}", "ă".repeat(80)));
    }

    #[test]
    fn scrub_hides_remote_and_paths() {
        let root = Path::new("/home/u/repo");
        let s = scrub(
            "To https://u:tok@git.example/x.git\n ! [rejected]\nerror: failed to push some refs to 'https://u:tok@git.example/x.git' from /home/u/repo",
            root,
            Some("https://u:tok@git.example/x.git"),
        );
        assert!(!s.contains("tok"), "{s}");
        assert!(!s.contains("/home/u/repo"), "{s}");
        assert!(s.contains("origin"), "{s}");
        let s = scrub("fatal: https://a:b@h/x denied", Path::new("/r"), None);
        assert_eq!(s, "fatal: https://***@h/x denied");
    }

    #[test]
    fn outcome_messages() {
        let mut o = MergePushOutcome {
            base: "dev".into(),
            branch: "task/x".into(),
            merge_commit: Some("abcdef0123".into()),
            pushed: true,
            branch_deleted: true,
            ..Default::default()
        };
        assert_eq!(
            o.message(),
            "Đã merge 'task/x' vào 'dev' (abcdef0) và push lên origin.\nĐã xoá nhánh cục bộ 'task/x'."
        );
        o.pushed = false;
        o.branch_deleted = false;
        o.push_failed = true;
        o.notes = vec!["Push lỗi: x".into()];
        assert_eq!(o.message(), "Đã merge 'task/x' vào 'dev' (abcdef0), chưa push.\nPush lỗi: x");
        let done = MergePushOutcome {
            base: "dev".into(),
            branch: "task/x".into(),
            pushed: true,
            already_done: true,
            ..Default::default()
        };
        assert_eq!(done.message(), "Nhánh 'task/x' đã merge vào 'dev' và push từ trước.");
    }

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
    fn default_worktree_mode_keeps_the_workplace_of_existing_tasks() {
        // A git project without a `workspace` key now defaults to worktree mode. Tasks created
        // before keep their place: a recorded workdir wins, and a task whose branch the repo has
        // checked out (how shared-mode tasks work) stays in the repo, with no new worktree.
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["commit", "-q", "--allow-empty", "-m", "init"]);
        run(&["checkout", "-q", "-b", "task/x"]);
        let project = Project::from_value(&serde_json::json!({
            "id": "web", "path": repo.to_string_lossy(), "name_vi": "web", "aliases": [],
            "type": "code", "repo": null, "profile": "work", "missing": false
        }))
        .unwrap();
        assert!(project.workspace.is_worktree());
        let conn = Connection::open_in_memory().unwrap();

        let wd = tmp.path().join("old-worktree");
        std::fs::create_dir_all(&wd).unwrap();
        let with_workdir = task("planned", Some(&wd.to_string_lossy()));
        assert_eq!(ensure_task_root(&conn, &project, &repo, &with_workdir).unwrap(), wd);

        let legacy = task("planned", None);
        assert_eq!(ensure_task_root(&conn, &project, &repo, &legacy).unwrap(), repo);
        assert_eq!(task_root(&project, &legacy), project.root());
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

    #[test]
    fn copy_vendor_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(repo.join("vendor/modules")).unwrap();
        std::fs::create_dir_all(repo.join("app-modules/x")).unwrap();
        std::fs::write(repo.join("composer.json"), "{}").unwrap();
        std::fs::write(repo.join("vendor/autoload.php"), "<?php // repo").unwrap();
        std::os::unix::fs::symlink("../../app-modules/x", repo.join("vendor/modules/x")).unwrap();

        std::fs::create_dir_all(wt.join("app-modules/x")).unwrap();

        // 1. Initial copy: autoload.php exists, relative symlink resolves inside worktree, tmp dir cleaned
        copy_vendor(&repo, &wt);
        assert!(wt.join("vendor/autoload.php").is_file());
        assert!(!wt.join(".vendor.agent-brain-tmp").exists());
        let resolved = std::fs::canonicalize(wt.join("vendor/modules/x")).unwrap();
        let expected_wt_app = std::fs::canonicalize(wt.join("app-modules/x")).unwrap();
        assert_eq!(resolved, expected_wt_app);

        // 2. Second call does not overwrite (idempotent, keeps old version)
        std::fs::write(repo.join("vendor/autoload.php"), "<?php // new").unwrap();
        copy_vendor(&repo, &wt);
        assert_eq!(
            std::fs::read_to_string(wt.join("vendor/autoload.php")).unwrap(),
            "<?php // repo"
        );

        // 3. Worktree has vendor as a symlink -> replaced with real dir
        let wt2 = tmp.path().join("wt2");
        std::fs::create_dir_all(wt2.join("app-modules/x")).unwrap();
        std::os::unix::fs::symlink(repo.join("vendor"), wt2.join("vendor")).unwrap();
        assert!(std::fs::symlink_metadata(wt2.join("vendor")).unwrap().file_type().is_symlink());
        copy_vendor(&repo, &wt2);
        assert!(std::fs::symlink_metadata(wt2.join("vendor")).unwrap().file_type().is_dir());
        assert!(!std::fs::symlink_metadata(wt2.join("vendor")).unwrap().file_type().is_symlink());
        assert!(wt2.join("vendor/autoload.php").is_file());

        // 4. Missing composer.json -> does not copy
        let repo_no_composer = tmp.path().join("repo_no_composer");
        let wt_no_composer = tmp.path().join("wt_no_composer");
        std::fs::create_dir_all(repo_no_composer.join("vendor")).unwrap();
        std::fs::create_dir_all(&wt_no_composer).unwrap();
        copy_vendor(&repo_no_composer, &wt_no_composer);
        assert!(!wt_no_composer.join("vendor").exists());

        // 5. Git status --porcelain in real worktree ignores vendor
        let git_run = |dir: &Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            out
        };
        let git_repo = tmp.path().join("git_repo");
        std::fs::create_dir_all(git_repo.join("vendor")).unwrap();
        std::fs::write(git_repo.join("composer.json"), "{}").unwrap();
        std::fs::write(git_repo.join("vendor/autoload.php"), "<?php").unwrap();
        git_run(&git_repo, &["init", "-q", "-b", "main"]);
        git_run(&git_repo, &["add", "composer.json"]);
        git_run(&git_repo, &["commit", "-q", "-m", "init"]);
        let git_wt = tmp.path().join("git_wt");
        git_run(&git_repo, &["worktree", "add", "-q", &git_wt.to_string_lossy(), "-b", "task/test"]);
        copy_vendor(&git_repo, &git_wt);
        let status = git_run(&git_wt, &["status", "--porcelain"]);
        let status_out = String::from_utf8_lossy(&status.stdout);
        assert!(!status_out.contains("vendor"), "git status should not show vendor, got: {status_out}");
    }
}
