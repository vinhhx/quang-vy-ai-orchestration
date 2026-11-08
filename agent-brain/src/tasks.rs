use crate::db::TaskRow;
use crate::error::BrainResult;
use rusqlite::Connection;
use std::path::Path;

pub const SUCCESS_STATUSES: &[&str] = &["answered", "qa_pass"];

pub fn is_success_status(status: &str) -> bool {
    SUCCESS_STATUSES.contains(&status)
}

/// The task of the branch checked out at `root`, for the default protocol layout.
pub fn current_task(
    conn: &Connection,
    project_row_id: i64,
    root: &Path,
) -> BrainResult<Option<TaskRow>> {
    current_task_in(conn, project_row_id, root, "")
}

/// The task of the branch checked out at `root`; a branch without a task row is adopted from
/// the plan file found under the project's `protocol_dir`.
pub fn current_task_in(
    conn: &Connection,
    project_row_id: i64,
    root: &Path,
    protocol_dir: &str,
) -> BrainResult<Option<TaskRow>> {
    let branch = match crate::gitops::current_branch(root) {
        Some(b) if b.starts_with("task/") => b,
        _ => return Ok(None),
    };

    if let Some(task) = crate::db::find_task_by_branch(conn, project_row_id, &branch)? {
        return Ok(Some(task));
    }

    let slug = branch.strip_prefix("task/").unwrap_or("");
    let plan_path = root.join(crate::protocol_paths::paths_in(protocol_dir, slug).plan);
    if plan_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&plan_path) {
            if let Some(meta) = crate::planfile::read_meta(&content) {
                if meta.get("branch").and_then(|s| s.as_str()) == Some(&branch) {
                    let slug = meta.get("task").and_then(|s| s.as_str()).unwrap_or("");
                    let title = meta
                        .get("request")
                        .and_then(|s| s.as_str())
                        .or_else(|| meta.get("task").and_then(|s| s.as_str()))
                        .unwrap_or("");
                    let base_branch = meta.get("base_branch").and_then(|s| s.as_str());
                    let base_commit = meta.get("base_commit").and_then(|s| s.as_str());

                    let id = crate::db::upsert_task(
                        conn,
                        project_row_id,
                        slug,
                        title,
                        &branch,
                        base_branch,
                        base_commit,
                    )?;
                    return crate::db::get_task(conn, id);
                }
            }
        }
    }

    Ok(None)
}
