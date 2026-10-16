use crate::db::TaskRow;
use crate::error::BrainResult;
use rusqlite::Connection;
use std::path::Path;

pub fn current_task(
    conn: &Connection,
    project_row_id: i64,
    root: &Path,
) -> BrainResult<Option<TaskRow>> {
    let branch = match crate::gitops::current_branch(root) {
        Some(b) if b.starts_with("task/") => b,
        _ => return Ok(None),
    };

    if let Some(task) = crate::db::find_task_by_branch(conn, project_row_id, &branch)? {
        return Ok(Some(task));
    }

    let plan_path = root.join(".plan.md");
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
