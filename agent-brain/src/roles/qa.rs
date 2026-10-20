use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub const QA_ALLOWED_TOOLS: [&str; 9] = [
    "Read",
    "Grep",
    "Glob",
    "Write(./.qa-report.md)",
    "Edit(./.qa-report.md)",
    "Bash(git diff *)",
    "Bash(git log *)",
    "Bash(git show *)",
    "Bash(git status *)",
];

pub const QA_DISALLOWED_TOOLS: [&str; 4] = ["WebFetch", "WebSearch", "NotebookEdit", "Task"];

pub fn cmd_qa(ctx: &mut Ctx, project: &str, opts: &crate::roles::RunOpts) -> BrainResult<i32> {
    let dry_run = opts.dry_run;
    let verbose = opts.verbose;
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
    dirty.remove(".qa-report.md");
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

    let code = crate::gitops::diff_code_names(&proj_path, &plan_commit, "HEAD")?;
    if code.is_empty() {
        return Err(BrainError::error("chưa có thay đổi code sau plan"));
    }

    let diff_cmd = format!(
        "git diff {plan_commit}..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md' ':(exclude).plan-review.md' ':(exclude).code-review.md'"
    );

    let mut allowed = QA_ALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    allowed.extend(crate::roles::abs_rules(
        &proj_path,
        &[".qa-report.md"],
        &["Write", "Edit"],
    ));

    let disallowed = QA_DISALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if dry_run {
        crate::term::emit_out(&format!("Diff command: {diff_cmd}"));
        let spec_dry = crate::roles::RoleSpec {
            role: crate::roles::Role::Qa,
            user_input: format!(
                "Task: {task}\nĐầu vào: .plan.md và kết quả lệnh: {diff_cmd}\nGhi .qa-report.md theo khung có sẵn; mục Kết luận ghi đúng một dòng: Đạt hoặc Cần sửa.\nỞ đầu .qa-report.md (trước dòng \"# QA:\") thêm front matter: dòng ---, specs: <số kiểm tra đã chạy>, failing: <số kiểm tra lỗi>, not_automatable: <số mục không kiểm tự động được>, dòng ---. Bỏ dòng nào bạn không biết."
            ),
            allowed_tools: allowed,
            disallowed_tools: disallowed,
            extra_args: Vec::new(),
        };
        let argv = crate::roles::build_command(&spec_dry, &ctx.cfg)?;
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&argv, "assets/prompts/qa.md")
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_row = crate::tasks::current_task(&ctx.conn, proj_row_id, &proj_path)?;
    let task_id = task_row.map(|t| t.id);

    let qa_path = proj_path.join(".qa-report.md");
    let qa_meta = serde_json::json!({
        "task": task,
        "plan_commit": plan_commit,
    });
    let req = meta
        .as_ref()
        .and_then(|m| m.get("request"))
        .and_then(|v| v.as_str())
        .unwrap_or(&task);
    std::fs::write(&qa_path, crate::planfile::render_qa_skeleton(req, &qa_meta))?;

    let user_input = format!(
        "Task: {task}\nĐầu vào: .plan.md và kết quả lệnh: {diff_cmd}\nGhi .qa-report.md theo khung có sẵn; mục Kết luận ghi đúng một dòng: Đạt hoặc Cần sửa.\nỞ đầu .qa-report.md (trước dòng \"# QA:\") thêm front matter: dòng ---, specs: <số kiểm tra đã chạy>, failing: <số kiểm tra lỗi>, not_automatable: <số mục không kiểm tự động được>, dòng ---. Bỏ dòng nào bạn không biết."
    );

    let spec = crate::roles::RoleSpec {
        role: crate::roles::Role::Qa,
        user_input,
        allowed_tools: allowed,
        disallowed_tools: disallowed,
        extra_args: Vec::new(),
    };

    let argv = crate::roles::build_command(&spec, &ctx.cfg)?;
    let model = ctx.cfg.model_for("qa")?;
    let timeout = ctx.cfg.timeout_for("qa");

    let run_plan = crate::roles::RunPlan {
        role: "qa".to_string(),
        model,
        argv,
        task_id,
        task_slug: task.clone(),
        base_commit: Some(plan_commit.clone()),
        plan_files: None,
        verbose,
        timeout,
        run_dir: None,
    };

    let role_run = crate::roles::execute_role_staged(
        ctx,
        &proj,
        &proj_path,
        run_plan,
        crate::roles::RunStage {
            stage: Some("qa".to_string()),
            round: opts.round,
            metrics: None,
        },
    )?;

    match role_run.outcome.status {
        crate::runner::RunStatus::TimedOut => {
            return Err(BrainError::error(format!(
                "QA chạy quá thời gian (timeout) trên task '{task}'"
            )));
        }
        crate::runner::RunStatus::Killed => {
            return Err(BrainError::new(
                "QA bị ngắt bởi người dùng",
                crate::error::EXIT_INTERRUPTED,
            ));
        }
        crate::runner::RunStatus::Failed => {
            let code = role_run.outcome.exit_code.unwrap_or(1);
            return Err(BrainError::error(format!(
                "QA thất bại với mã lỗi {code} trên task '{task}'"
            )));
        }
        _ => {
            if let Some(code) = role_run.outcome.exit_code {
                if code != 0 {
                    return Err(BrainError::error(format!(
                        "QA thất bại với mã lỗi {code} trên task '{task}'"
                    )));
                }
            }
        }
    }

    let mut extra = crate::gitops::changed_paths(&proj_path)?;
    extra.remove(".qa-report.md");
    if !extra.is_empty() {
        crate::term::emit_out("QA đã sửa file ngoài .qa-report.md — vi phạm giao thức (ROLE-04):");
        let mut sorted_extra: Vec<String> = extra.into_iter().collect();
        sorted_extra.sort();
        for p in sorted_extra {
            crate::term::emit_out(&format!("  {p}"));
        }
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !qa_path.is_file() {
        return Err(BrainError::error("QA không tạo file .qa-report.md"));
    }

    let qa_text = std::fs::read_to_string(&qa_path)?;
    let verdict = crate::planfile::parse_qa_verdict(&qa_text);
    let verdict = match verdict {
        Some(v) => v,
        None => {
            return Err(BrainError::error("QA chưa ghi Kết luận (Đạt / Cần sửa)"));
        }
    };

    crate::gitops::commit_paths(&proj_path, &[".qa-report.md"], &format!("{task}: qa"))?;
    crate::term::emit_out(&format!("Kết luận QA: {verdict}"));

    crate::db::merge_run_metrics(
        &ctx.conn,
        role_run.run_id,
        &crate::report::qa_metrics(&crate::report::parse_qa_counts(&qa_text)),
    )?;

    if verdict == "Cần sửa" {
        crate::term::emit_out("Bước tiếp theo:");
        crate::term::emit_out(&format!(
            "- Sửa .plan.md (commit '{task}: plan') hoặc chạy lại agent-brain run lead \"{project}\" \"{req}\" --name {task}"
        ));
        crate::term::emit_out(&format!("- Rồi chạy agent-brain run dev \"{project}\""));
        crate::term::emit_out(&format!(
            "- Hoặc sửa theo báo cáo QA: agent-brain run dev \"{project}\" --fix qa"
        ));
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "qa:fix")?;
        if let Some(tid) = task_id {
            crate::db::set_task_status(&ctx.conn, tid, "qa_fix")?;
        }
    } else {
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "qa:pass")?;
        if let Some(tid) = task_id {
            crate::db::set_task_status(&ctx.conn, tid, "qa_pass")?;
        }
    }

    Ok(crate::error::EXIT_OK)
}
