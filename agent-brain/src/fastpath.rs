use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::term;

pub fn cmd_status(ctx: &mut Ctx, project_query: &str) -> BrainResult<i32> {
    let project = crate::projects::select_project(project_query, &ctx.map)?;
    let root = project.root();

    if !root.is_dir() {
        return Err(BrainError::error(format!(
            "Đường dẫn dự án '{}' không tồn tại: {}",
            project.id,
            root.display()
        )));
    }
    if !crate::gitops::is_repo_root(&root) {
        return Err(BrainError::error(format!(
            "Thư mục dự án '{}' không phải repo git: {}",
            project.id,
            root.display()
        )));
    }

    term::emit_out(&format!("Dự án: {} ({})", project.id, root.display()));

    let branch = crate::gitops::current_branch(&root).unwrap_or_else(|| "(detached)".to_string());
    term::emit_out(&format!("Nhánh: {branch}"));

    term::emit_out("== git status --short ==");
    let status_out = crate::gitops::git(&root, &["status", "--short"])?;
    if status_out.trim().is_empty() {
        term::emit_out("(sạch)");
    } else {
        term::emit_out(status_out.trim_end());
    }

    term::emit_out("== git log --oneline -5 ==");
    let log_res = crate::gitops::git(&root, &["log", "--oneline", "-5"]);
    match log_res {
        Ok(ref s) if !s.trim().is_empty() => term::emit_out(s.trim_end()),
        _ => term::emit_out("(chưa có commit)"),
    }

    let project_row_id = crate::db::project_row_id(&ctx.conn, &project.id)?;
    let task_opt = crate::tasks::current_task(&ctx.conn, project_row_id, &root)?;

    match task_opt {
        Some(task) => {
            let base_commit = task.base_commit.as_deref().unwrap_or("HEAD");
            let short_base = if base_commit.len() >= 7 {
                &base_commit[..7]
            } else {
                base_commit
            };
            term::emit_out(&format!("== git diff --stat {short_base} =="));
            let diff_res = crate::gitops::git(&root, &["diff", "--stat", base_commit]);
            match diff_res {
                Ok(ref d) if !d.trim().is_empty() => term::emit_out(d.trim_end()),
                _ => term::emit_out("(không có thay đổi)"),
            }

            term::emit_out(&format!(
                "Task {} · {} · trạng thái {}",
                task.id, task.branch, task.status
            ));

            let latest_run = crate::db::latest_run_for_task(&ctx.conn, task.id)?;
            match latest_run {
                Some(run) => {
                    term::emit_out(&format!(
                        "Run gần nhất: run {} · {} · {} · token vào {}, ra {}",
                        run.id, run.role, run.status, run.input_tokens, run.output_tokens
                    ));
                }
                None => {
                    term::emit_out("Run gần nhất: chưa có");
                }
            }
        }
        None => {
            term::emit_out("Không ở nhánh task — bỏ qua diff so với commit base");
        }
    }

    Ok(0)
}

pub fn cmd_inspect(_ctx: &mut Ctx, _project: &str, _verbose: bool) -> BrainResult<i32> {
    Err(BrainError::not_available("inspect"))
}
