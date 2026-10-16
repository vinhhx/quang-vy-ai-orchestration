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

pub const INSPECT_PROMPT_HEADER: &str = "Review ngắn bản thay đổi dưới đây, trả lời bằng tiếng Việt, tối đa 15 dòng: (1) tóm tắt thay đổi; (2) rủi ro dễ thấy; (3) có khớp .plan.md không (nếu có .plan.md bên dưới). Không dùng công cụ nào.\n\n";
pub const INSPECT_HARD_CAP: usize = 120_000;

pub fn build_inspect_prompt(
    _plan: Option<&str>,
    _diff: &str,
    _base: &str,
    _cap: usize,
) -> (String, Option<(usize, usize)>) {
    (String::new(), None)
}

pub fn inspect_argv(_cfg: &crate::config::Config, _prompt: &str) -> BrainResult<Vec<String>> {
    Err(BrainError::not_available("inspect_argv"))
}

pub fn is_session_uuid(_s: &str) -> bool {
    false
}

pub fn cleanup_session_files(
    _config_dir: &std::path::Path,
    _session_id: &str,
) -> Vec<std::path::PathBuf> {
    Vec::new()
}

pub fn cmd_inspect(_ctx: &mut Ctx, _project: &str, _verbose: bool) -> BrainResult<i32> {
    Err(BrainError::not_available("inspect"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_inspect_prompt_cap_boundary() {
        let base = "HEAD";
        let plan = None;
        let prefix = format!("{INSPECT_PROMPT_HEADER}--- git diff {base} ---\n");
        let diff = "line 1\nline 2\nline 3\n";
        let total_len = prefix.len() + diff.len();

        let (p1, cut1) = build_inspect_prompt(plan, diff, base, total_len);
        assert_eq!(cut1, None);
        assert_eq!(p1.len(), total_len);
        assert!(!p1.contains("[diff cắt"));

        let cap2 = total_len - 1;
        let (p2, cut2) = build_inspect_prompt(plan, diff, base, cap2);
        assert!(cut2.is_some());
        let (sent, total) = cut2.unwrap();
        assert_eq!(total, diff.len());
        assert!(sent < total);
        assert!(p2.len() <= cap2);
        assert!(p2.contains("[diff cắt: đã gửi "));
    }

    #[test]
    fn test_is_session_uuid() {
        assert!(is_session_uuid("d2edd3e2-1234-5678-9abc-def012345678"));
        assert!(!is_session_uuid("D2EDD3E2-1234-5678-9ABC-DEF012345678"));
        assert!(!is_session_uuid("../../etc"));
        assert!(!is_session_uuid(""));
        assert!(!is_session_uuid("d2edd3e2123456789abcdef012345678"));
    }

    #[test]
    fn test_cleanup_session_files_symlink_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path();
        let projects_dir = config_dir.join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();

        let uuid = "11111111-2222-3333-4444-555555555555";
        let outside_file = outside.join(format!("{uuid}.jsonl"));
        std::fs::write(&outside_file, "secret").unwrap();

        let symlink_proj = projects_dir.join("symlink-proj");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &symlink_proj).unwrap();

        let removed = cleanup_session_files(config_dir, uuid);
        assert!(removed.is_empty(), "cleanup must not follow symlinks");
        assert!(outside_file.exists(), "outside file must not be deleted");
    }
}
