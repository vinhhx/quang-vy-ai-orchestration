use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::report::ReviewKind;
use crate::roles::RunOpts;
use unicode_normalization::UnicodeNormalization;

pub const REVIEW_ALLOWED_BASE: [&str; 7] = [
    "Read",
    "Grep",
    "Glob",
    "Bash(git diff *)",
    "Bash(git log *)",
    "Bash(git show *)",
    "Bash(git status *)",
];

pub const REVIEW_DISALLOWED_TOOLS: [&str; 4] = ["WebFetch", "WebSearch", "NotebookEdit", "Task"];

pub fn render_review_skeleton(kind: ReviewKind, title: &str, meta: &serde_json::Value) -> String {
    let raw_first_line = title.lines().next().unwrap_or(title);
    let title_100: String = raw_first_line.chars().take(100).collect();
    let norm_title: String = title_100.nfc().collect();
    let template = match kind {
        ReviewKind::Plan => include_str!("../../assets/templates/plan-review.md"),
        ReviewKind::Code => include_str!("../../assets/templates/code-review.md"),
    };
    let lines: Vec<&str> = template.lines().collect();
    let mut new_lines = Vec::with_capacity(lines.len() + 1);
    let ml = crate::planfile::meta_line(meta);

    for line in lines {
        if line.contains("<tên task>") {
            let replaced = line.replace("<tên task>", &norm_title);
            new_lines.push(replaced);
            new_lines.push(ml.clone());
        } else {
            new_lines.push(line.to_string());
        }
    }

    new_lines.join("\n") + "\n"
}

pub fn cmd_review(
    ctx: &mut Ctx,
    project: &str,
    kind: ReviewKind,
    opts: &RunOpts,
) -> BrainResult<i32> {
    let proj = crate::projects::select_project(project, &ctx.map)?;
    let proj_path = crate::projects::require_runnable(&proj, &ctx.cfg)?;

    let plan_path = proj_path.join(".plan.md");
    if !plan_path.is_file() {
        return Err(BrainError::error("Không tìm thấy .plan.md ở gốc dự án"));
    }

    let text_plan = std::fs::read_to_string(&plan_path)?;
    let meta = crate::planfile::read_meta(&text_plan);
    let task = match meta
        .as_ref()
        .and_then(|m| m.get("task"))
        .and_then(|v| v.as_str())
    {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Err(BrainError::error("File .plan.md không có metadata hợp lệ")),
    };

    let expected_branch = meta
        .as_ref()
        .and_then(|m| m.get("branch"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("task/{task}"));

    let curr_branch = crate::gitops::current_branch(&proj_path);
    let branch_ok = match &curr_branch {
        Some(b) => b.starts_with("task/") && b == &expected_branch,
        None => false,
    };
    if !branch_ok {
        let cur_display = curr_branch.as_deref().unwrap_or("None");
        return Err(BrainError::error(format!(
            "Không ở nhánh task '{expected_branch}' (nhánh hiện tại: '{cur_display}') — không đoán"
        )));
    }

    // Con của autopilot chỉ chạy trên nhánh của đúng task autopilot (CR-01).
    crate::roles::autopilot_task_guard(&ctx.conn, &proj.id, &proj_path)?;

    let mut dirty = crate::gitops::changed_paths(&proj_path)?;
    dirty.remove(kind.file());
    if !dirty.is_empty() {
        let mut sorted_dirty: Vec<String> = dirty.into_iter().collect();
        sorted_dirty.sort();
        let list = sorted_dirty
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BrainError::error(format!(
            "Working tree có thay đổi chưa commit:\n{list}"
        )));
    }

    let plan_commit = crate::gitops::find_commit_by_subject(&proj_path, &format!("{task}: plan"))?;
    let plan_commit = match plan_commit {
        Some(c) => c,
        None => {
            return Err(BrainError::error(format!(
                "Không tìm thấy commit '{task}: plan' trên nhánh hiện tại"
            )));
        }
    };

    if kind == ReviewKind::Code {
        let code = crate::gitops::diff_code_names(&proj_path, &plan_commit, "HEAD")?;
        if code.is_empty() {
            return Err(BrainError::error("chưa có thay đổi code sau plan"));
        }
    }

    let diff_cmd = format!(
        "git diff {plan_commit}..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md' ':(exclude).plan-review.md' ':(exclude).code-review.md'"
    );

    let file = kind.file();
    let mut allowed = REVIEW_ALLOWED_BASE
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    allowed.push(format!("Write(./{file})"));
    allowed.push(format!("Edit(./{file})"));
    allowed.extend(crate::roles::abs_rules(
        &proj_path,
        &[file],
        &["Write", "Edit"],
    ));

    let disallowed = REVIEW_DISALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let has_prev =
        crate::gitops::git(&proj_path, &["cat-file", "-e", &format!("HEAD:{file}")]).is_ok();
    let prev_line = if has_prev {
        format!("Báo cáo vòng trước: chạy git show HEAD:{file} để đọc trước khi ghi.\n")
    } else {
        String::new()
    };

    let user_input = match kind {
        ReviewKind::Plan => format!(
            "Task: {task}\nĐầu vào: .plan.md (commit {plan_commit})\n{prev_line}Ghi {file} theo khung có sẵn: điền front matter verdict / blocking / advisory / files ở đầu file; verdict là revise khi và chỉ khi có ít nhất một mục chặn."
        ),
        ReviewKind::Code => format!(
            "Task: {task}\nĐầu vào: .plan.md và kết quả lệnh: {diff_cmd}\n{prev_line}Ghi {file} theo khung có sẵn: điền front matter verdict / blocking / advisory / files ở đầu file; verdict là revise khi và chỉ khi có ít nhất một mục chặn."
        ),
    };

    let role_enum = match kind {
        ReviewKind::Plan => crate::roles::Role::PlanReview,
        ReviewKind::Code => crate::roles::Role::CodeReview,
    };

    if opts.dry_run {
        if kind == ReviewKind::Code {
            crate::term::emit_out(&format!("Diff command: {diff_cmd}"));
        }
        let spec_dry = crate::roles::RoleSpec {
            role: role_enum,
            user_input: user_input.clone(),
            allowed_tools: allowed,
            disallowed_tools: disallowed,
            extra_args: vec![
                "--settings".to_string(),
                "<run_dir/settings.json>".to_string(),
            ],
        };
        let argv = crate::roles::build_command(&spec_dry, &ctx.cfg)?;
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&argv, &format!("assets/prompts/{}.md", kind.role()))
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_row = crate::tasks::current_task(&ctx.conn, proj_row_id, &proj_path)?;
    let task_id = task_row.map(|t| t.id);

    let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task, kind.role())?;
    let exe = std::env::current_exe()?;
    let settings = crate::guard::write_hook_settings(
        &run_dir,
        &proj_path,
        None,
        &[proj_path.join(kind.file())],
        &exe,
    )?;

    let report_path = proj_path.join(kind.file());
    let review_meta = serde_json::json!({
        "task": task,
        "plan_commit": plan_commit,
        "round": opts.round,
    });
    let req = meta
        .as_ref()
        .and_then(|m| m.get("request"))
        .and_then(|v| v.as_str())
        .unwrap_or(&task);
    std::fs::write(
        &report_path,
        render_review_skeleton(kind, req, &review_meta),
    )?;

    let spec = crate::roles::RoleSpec {
        role: role_enum,
        user_input,
        allowed_tools: allowed,
        disallowed_tools: disallowed,
        extra_args: vec![
            "--settings".to_string(),
            settings.to_string_lossy().to_string(),
        ],
    };

    let argv = crate::roles::build_command(&spec, &ctx.cfg)?;
    let model = ctx.cfg.model_for(kind.role())?;
    let timeout = ctx.cfg.timeout_for(kind.role());

    let run_plan = crate::roles::RunPlan {
        role: kind.role().to_string(),
        model,
        argv,
        task_id,
        task_slug: task.clone(),
        base_commit: Some(plan_commit.clone()),
        plan_files: None,
        verbose: opts.verbose,
        timeout,
        run_dir: Some(run_dir),
    };

    let stage = crate::roles::RunStage {
        stage: Some(kind.role().to_string()),
        round: opts.round,
        metrics: None,
    };

    let role_run = crate::roles::execute_role_staged(ctx, &proj, &proj_path, run_plan, stage)?;

    let title = kind.title();
    match role_run.outcome.status {
        crate::runner::RunStatus::TimedOut => {
            return Err(BrainError::error(format!(
                "{title} chạy quá thời gian (timeout) trên task '{task}'"
            )));
        }
        crate::runner::RunStatus::Killed => {
            return Err(BrainError::new(
                format!("{title} bị ngắt bởi người dùng"),
                crate::error::EXIT_INTERRUPTED,
            ));
        }
        crate::runner::RunStatus::Failed => {
            let code = role_run.outcome.exit_code.unwrap_or(1);
            return Err(BrainError::error(format!(
                "{title} thất bại với mã lỗi {code} trên task '{task}'"
            )));
        }
        _ => {
            if let Some(code) = role_run.outcome.exit_code {
                if code != 0 {
                    return Err(BrainError::error(format!(
                        "{title} thất bại với mã lỗi {code} trên task '{task}'"
                    )));
                }
            }
        }
    }

    let mut extra = crate::gitops::changed_paths(&proj_path)?;
    extra.remove(kind.file());
    if !extra.is_empty() {
        crate::term::emit_out(&format!(
            "{title} đã sửa file ngoài {} — vi phạm giao thức:",
            kind.file()
        ));
        let mut sorted_extra: Vec<String> = extra.into_iter().collect();
        sorted_extra.sort();
        for p in sorted_extra {
            crate::term::emit_out(&format!("  {p}"));
        }
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !report_path.is_file() {
        return Err(BrainError::error(format!(
            "{title} không tạo file {}",
            kind.file()
        )));
    }

    let report_text = std::fs::read_to_string(&report_path)?;
    let report = match crate::report::parse_review(&report_text, kind) {
        Ok(r) => r,
        Err(reason) => {
            crate::term::emit_out(&format!(
                "Báo cáo {} không đúng định dạng: {reason}",
                kind.file()
            ));
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
            return Ok(crate::error::EXIT_PROTOCOL);
        }
    };

    let report_changed = crate::gitops::changed_paths(&proj_path)?.contains(kind.file());
    if report_changed {
        crate::gitops::commit_paths(
            &proj_path,
            &[kind.file()],
            &format!("{task}: {}", kind.role()),
        )?;
    } else {
        crate::term::emit_out(&format!(
            "Báo cáo {} giống bản đã commit — không tạo commit mới.",
            kind.file()
        ));
    }

    let metrics = crate::report::review_metrics(&report);
    crate::db::merge_run_metrics(&ctx.conn, role_run.run_id, &metrics)?;
    let verdict = &report.verdict;
    let b = report.blocking;
    let a = report.advisory;
    crate::db::set_run_outcome(&ctx.conn, role_run.run_id, &format!("review:{verdict}"))?;

    crate::term::emit_out(&format!(
        "Kết luận {title}: {verdict} ({b} chặn, {a} góp ý)"
    ));
    if verdict == "revise" {
        crate::term::emit_out(&format!(
            "Bước tiếp theo: sửa theo {} rồi chạy lại review",
            kind.file()
        ));
    }

    Ok(crate::error::EXIT_OK)
}
