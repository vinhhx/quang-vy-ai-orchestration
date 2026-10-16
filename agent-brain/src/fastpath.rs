use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::term;
use std::path::{Path, PathBuf};

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
    plan: Option<&str>,
    diff: &str,
    base: &str,
    cap: usize,
) -> (String, Option<(usize, usize)>) {
    let mut header = String::from(INSPECT_PROMPT_HEADER);
    if let Some(p) = plan {
        header.push_str("--- .plan.md ---\n");
        if p.len() <= 20_000 {
            header.push_str(p);
            if !p.ends_with('\n') {
                header.push('\n');
            }
        } else {
            let cut_notice = "[plan cắt]\n";
            let target_limit = 20_000usize.saturating_sub(cut_notice.len());
            let cut_idx = p[..target_limit].rfind('\n').map(|i| i + 1).unwrap_or(0);
            header.push_str(&p[..cut_idx]);
            header.push_str(cut_notice);
        }
    }
    header.push_str(&format!("--- git diff {base} ---\n"));

    let prefix_len = header.len();
    let m = diff.len();
    if prefix_len + m <= cap {
        let mut prompt = header;
        prompt.push_str(diff);
        return (prompt, None);
    }

    let max_n = cap.saturating_sub(prefix_len).min(m);
    let bytes = diff.as_bytes();
    let mut best_n = 0;
    let mut best_notice = String::new();

    let mut idx = max_n;
    while idx > 0 {
        if bytes[idx - 1] == b'\n' {
            let notice = format!("[diff cắt: đã gửi {idx}/{m} byte]");
            if prefix_len + idx + notice.len() <= cap {
                best_n = idx;
                best_notice = notice;
                break;
            }
        }
        idx -= 1;
    }

    if best_n == 0 {
        best_notice = format!("[diff cắt: đã gửi 0/{m} byte]");
    }

    let mut prompt = header;
    prompt.push_str(&diff[..best_n]);
    prompt.push_str(&best_notice);
    (prompt, Some((best_n, m)))
}

pub fn inspect_argv(cfg: &crate::config::Config, prompt: &str) -> BrainResult<Vec<String>> {
    crate::roles::check_forbidden_args(&cfg.common_args)?;
    let mut argv = vec![
        cfg.claude_bin.clone(),
        "-p".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--model".to_string(),
        cfg.inspect_model.clone(),
        "--max-turns".to_string(),
        "1".to_string(),
        "--tools".to_string(),
        "".to_string(),
        "--no-session-persistence".to_string(),
    ];
    argv.extend(cfg.common_args.iter().cloned());
    Ok(argv)
}

pub fn is_session_uuid(s: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
            .unwrap()
    });
    re.is_match(s)
}

pub fn cleanup_session_files(config_dir: &Path, session_id: &str) -> Vec<PathBuf> {
    if !is_session_uuid(session_id) {
        return Vec::new();
    }
    let projects_dir = config_dir.join("projects");
    let entries = match std::fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut removed = Vec::new();
    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        let meta = match std::fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            continue;
        }

        let jsonl_file = p.join(format!("{session_id}.jsonl"));
        if let Ok(m) = std::fs::symlink_metadata(&jsonl_file) {
            if m.is_file() && !m.file_type().is_symlink() {
                if std::fs::remove_file(&jsonl_file).is_ok() {
                    removed.push(jsonl_file);
                }
            }
        }

        let session_dir = p.join(session_id);
        if let Ok(m) = std::fs::symlink_metadata(&session_dir) {
            if m.is_dir() && !m.file_type().is_symlink() {
                if std::fs::remove_dir_all(&session_dir).is_ok() {
                    removed.push(session_dir);
                }
            }
        }
    }
    removed
}

pub fn cmd_inspect(ctx: &mut Ctx, project: &str, verbose: bool) -> BrainResult<i32> {
    let project = crate::projects::select_project(project, &ctx.map)?;
    let root = crate::projects::require_runnable(&project, &ctx.cfg)?;
    let project_row_id = crate::db::project_row_id(&ctx.conn, &project.id)?;
    let task = crate::tasks::current_task(&ctx.conn, project_row_id, &root)?;

    let (base, task_id, task_slug) = match &task {
        Some(t) => (
            t.base_commit.as_deref().unwrap_or("HEAD"),
            Some(t.id),
            t.slug.clone(),
        ),
        None => ("HEAD", None, "inspect".to_string()),
    };

    let plan_path = root.join(".plan.md");
    let plan_content = if plan_path.is_file() {
        std::fs::read_to_string(&plan_path).ok()
    } else {
        None
    };

    let diff = crate::gitops::git(
        &root,
        &[
            "diff",
            base,
            "--",
            ".",
            ":(exclude).plan.md",
            ":(exclude).qa-report.md",
        ],
    )?;

    if diff.trim().is_empty() {
        term::emit_out(&format!("Không có thay đổi so với {base} — không gọi AI"));
        return Ok(0);
    }

    let cap = ctx.cfg.inspect_max_diff_bytes.min(INSPECT_HARD_CAP);
    let (prompt, cut_info) = build_inspect_prompt(plan_content.as_deref(), &diff, base, cap);
    if let Some((sent, total)) = cut_info {
        term::emit(&format!("Diff quá lớn — đã cắt: gửi {sent}/{total} byte"));
    }

    let argv = inspect_argv(&ctx.cfg, &prompt)?;
    let resolved_base_sha = crate::gitops::git(&root, &["rev-parse", base])
        .ok()
        .map(|s| s.trim().to_string());

    let run_plan = crate::roles::RunPlan {
        role: "inspect".to_string(),
        model: ctx.cfg.inspect_model.clone(),
        argv,
        task_id,
        task_slug,
        base_commit: resolved_base_sha,
        plan_files: None,
        verbose,
        timeout: ctx.cfg.timeout_for("inspect"),
        run_dir: None,
    };

    let role_run = crate::roles::execute_role(ctx, &project, &root, run_plan)?;

    if let Some(ref text) = role_run.summary.result_text {
        term::emit_out(text);
    }

    // Cleanup session files
    let env = crate::profile::build_env(
        &project.profile,
        &ctx.cfg.profiles,
        &crate::profile::current_env(),
    )?;
    let config_dir = match env.get("CLAUDE_CONFIG_DIR") {
        Some(d) => PathBuf::from(d),
        None => crate::config::home_dir().join(".claude"),
    };
    let removed = cleanup_session_files(&config_dir, &role_run.summary.session_id);
    for p in removed {
        term::emit(&format!("Đã xóa file session còn sót: {}", p.display()));
    }

    if role_run.outcome.status == crate::runner::RunStatus::Killed {
        return Ok(130);
    }

    let is_success = role_run.outcome.status == crate::runner::RunStatus::Succeeded
        && role_run.summary.subtype.as_deref() == Some("success")
        && role_run.summary.num_turns == Some(1);

    if is_success {
        Ok(0)
    } else {
        let status_str = role_run.outcome.status.as_str();
        let subtype_str = role_run.summary.subtype.as_deref().unwrap_or("-");
        let num_turns_str = role_run
            .summary
            .num_turns
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".to_string());
        term::emit(&format!(
            "inspect không dùng được: trạng thái {status_str}, subtype {subtype_str}, num_turns {num_turns_str} (cần success và đúng 1 turn)"
        ));
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_inspect_prompt_cap_boundary() {
        let base = "HEAD";
        let plan = None;
        let prefix = format!("{INSPECT_PROMPT_HEADER}--- git diff {base} ---\n");
        let diff = "123456789\n".repeat(50); // 500 bytes
        let total_len = prefix.len() + diff.len();

        let (p1, cut1) = build_inspect_prompt(plan, &diff, base, total_len);
        assert_eq!(cut1, None);
        assert_eq!(p1.len(), total_len);
        assert!(!p1.contains("[diff cắt"));

        let cap2 = total_len - 1;
        let (p2, cut2) = build_inspect_prompt(plan, &diff, base, cap2);
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
