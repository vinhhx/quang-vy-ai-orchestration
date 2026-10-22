use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub const LEAD_ALLOWED_TOOLS: [&str; 9] = [
    "Read",
    "Grep",
    "Glob",
    "Write(./.plan.md)",
    "Edit(./.plan.md)",
    "Bash(git log *)",
    "Bash(git show *)",
    "Bash(git diff *)",
    "Bash(git status *)",
];

pub const LEAD_DISALLOWED_TOOLS: [&str; 4] = ["WebFetch", "WebSearch", "NotebookEdit", "Task"];
pub const MAX_NOTE_CHARS: usize = 500;

/// IN-02 / CONTEXT O7: round của một Lead chạy tay (không có `--round`) = round hiệu lực
/// mà `effective_rounds` tính cho run mới khi thêm nó vào cuối các run của task.
fn manual_round(conn: &rusqlite::Connection, task_id: i64) -> BrainResult<i64> {
    use crate::autopilot::decide::{effective_rounds, RunFact, Stage};

    let rows = crate::db::runs_for_task(conn, task_id)?;
    let mut facts: Vec<RunFact> = rows
        .iter()
        .filter(|r| r.role != "inspect")
        .map(RunFact::from_row)
        .collect();
    facts.sort_by_key(|r| (r.started_at, r.id));
    let next_id = rows.iter().map(|r| r.id).max().unwrap_or(0) + 1;
    facts.push(RunFact {
        id: next_id,
        role: "lead".to_string(),
        stage: Some(Stage::Plan),
        fix_for: None,
        round: None,
        status: "running".to_string(),
        outcome: None,
        kill_requested: false,
        started_at: crate::db::now_ms(),
    });
    Ok(effective_rounds(&facts).last().copied().unwrap_or(1))
}

pub fn cmd_lead(
    ctx: &mut Ctx,
    project: &str,
    request: &str,
    opts: &crate::roles::RunOpts,
) -> BrainResult<i32> {
    let note = opts
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(n) = note {
        if n.chars().count() > MAX_NOTE_CHARS {
            return Err(BrainError::error("Ghi chú quá dài (tối đa 500 ký tự)"));
        }
    }

    let proj = crate::projects::select_project(project, &ctx.map)?;
    let proj_path = crate::projects::require_runnable(&proj, &ctx.cfg)?;

    let task_slug = crate::routing::slugify(opts.name.as_deref().unwrap_or(request));
    if task_slug.is_empty() {
        return Err(BrainError::error(
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
        ));
    }
    let branch = format!("task/{task_slug}");

    let loaded = crate::workflows::load(&ctx.cfg);
    for e in &loaded.errors {
        crate::term::emit(&format!("Cảnh báo workflows.json: {e}"));
    }
    let resolved = crate::workflows::resolve(
        &loaded,
        opts.workflow.as_deref(),
        crate::projects::workflow_of(&ctx.map, &proj.id).as_deref(),
    )?;
    if let Some(ref note_msg) = resolved.note {
        crate::term::emit(note_msg);
    }

    let curr_branch = crate::gitops::current_branch(&proj_path);
    let is_replan = curr_branch.as_deref() == Some(&branch);

    if opts.review {
        if !is_replan {
            return Err(BrainError::error(format!(
                "--review chỉ dùng khi đang ở nhánh {branch}"
            )));
        }
        let review_file = proj_path.join(crate::report::PLAN_REVIEW_FILE);
        let valid_revise = if review_file.is_file() {
            if let Ok(text) = std::fs::read_to_string(&review_file) {
                crate::report::parse_review(&text, crate::report::ReviewKind::Plan)
                    .map(|r| r.verdict == "revise")
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };
        if !valid_revise {
            return Err(BrainError::error(
                "--review cần .plan-review.md có verdict revise (chạy agent-brain run plan-review trước)",
            ));
        }
    }

    let plan_path = proj_path.join(".plan.md");
    let keep_plan = is_replan && (opts.review || note.is_some()) && plan_path.is_file();

    let user_input = if keep_plan {
        let mut s = format!(
            "Yêu cầu: {}\nTask: {}\nFile .plan.md ở gốc dự án là bản plan hiện tại; sửa trực tiếp trên bản này, giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo).",
            request, task_slug
        );
        if opts.review {
            s.push_str("\nBáo cáo review plan: .plan-review.md (verdict revise). Sửa .plan.md để xử lý mọi mục chặn trong báo cáo.");
        }
        if let Some(n) = note {
            s.push_str(&format!("\nGhi chú của người dùng: {n}"));
        }
        s
    } else {
        let mut s = format!(
            "Yêu cầu: {}\nTask: {}\nFile .plan.md ở gốc dự án đã có sẵn khung (skeleton). Giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo). Ghi kế hoạch vào .plan.md theo đúng mẫu.",
            request, task_slug
        );
        if let Some(n) = note {
            s.push_str(&format!("\nGhi chú của người dùng: {n}"));
        }
        s
    };

    let abs_plan_rules = crate::roles::abs_rules(&proj_path, &[".plan.md"], &["Write", "Edit"]);
    let mut allowed = LEAD_ALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    allowed.extend(abs_plan_rules);

    let disallowed = LEAD_DISALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if opts.dry_run {
        let spec_dry = crate::roles::RoleSpec {
            role: crate::roles::Role::Lead,
            user_input,
            allowed_tools: allowed,
            disallowed_tools: disallowed,
            extra_args: vec![
                "--settings".to_string(),
                "<run_dir/settings.json>".to_string(),
            ],
        };
        let argv = crate::roles::build_command(&spec_dry, &ctx.cfg)?;
        let p_prof = &proj.profile;
        let p_desc = crate::profile::describe_profile(p_prof, &ctx.cfg.profiles);
        let model = ctx.cfg.model_for("lead")?;

        crate::term::emit_out(&format!("Project: {}", proj.id));
        crate::term::emit_out(&format!("Profile: {p_prof} ({p_desc})"));
        crate::term::emit_out(&format!("Model: {model}"));
        crate::term::emit_out(&format!("Branch: {branch}"));
        crate::term::emit_out(&format!(
            "Workflow: {} — {}",
            resolved.workflow.name,
            resolved.workflow.summary()
        ));
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&argv, "assets/prompts/lead.md")
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let mut dirty = crate::gitops::changed_paths(&proj_path)?;
    if crate::gitops::current_branch(&proj_path).as_deref() == Some(&branch) {
        dirty.remove(".plan.md");
    }
    if !dirty.is_empty() {
        let mut list: Vec<String> = dirty.into_iter().collect();
        list.sort();
        return Err(BrainError::error(format!(
            "Working tree có thay đổi chưa commit — dừng, không tự stash:\n{}",
            list.join("\n")
        )));
    }

    let (base_branch, base_commit, created_at) = if is_replan {
        let mut ex_meta = None;
        if plan_path.is_file() {
            if let Ok(text) = std::fs::read_to_string(&plan_path) {
                ex_meta = crate::planfile::read_meta(&text);
            }
        }
        if let Some(m) = ex_meta {
            (
                m.get("base_branch")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                m.get("base_commit")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                m.get("created_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            )
        } else {
            (
                None,
                Some(crate::gitops::head_sha(&proj_path)?),
                chrono::Utc::now().to_rfc3339(),
            )
        }
    } else if crate::gitops::branch_exists(&proj_path, &branch) {
        return Err(BrainError::error(format!(
            "Nhánh '{branch}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác"
        )));
    } else {
        let bb = curr_branch.clone();
        let bc = crate::gitops::head_sha(&proj_path)?;
        let ca = chrono::Utc::now().to_rfc3339();
        if let Some(ref cb) = curr_branch {
            if cb.starts_with("task/") {
                crate::term::emit(&format!(
                    "Cảnh báo: Nhánh mới '{branch}' được tạo từ nhánh task '{cb}' (stacking)"
                ));
            }
        }
        (bb, Some(bc), ca)
    };

    let meta = serde_json::json!({
        "task": task_slug,
        "branch": branch,
        "base_branch": base_branch,
        "base_commit": base_commit,
        "request": request,
        "created_at": created_at,
    });

    if !is_replan {
        crate::gitops::create_branch(&proj_path, &branch)?;
    }

    if keep_plan {
        let text = std::fs::read_to_string(&plan_path)?;
        let curr_m = crate::planfile::read_meta(&text);
        if curr_m.as_ref() != Some(&meta) {
            let updated = crate::planfile::set_meta(&text, &meta);
            std::fs::write(&plan_path, updated)?;
        }
    } else {
        let skeleton = crate::planfile::render_plan_skeleton(request, &meta);
        std::fs::write(&plan_path, skeleton)?;
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_id = crate::db::upsert_task(
        &ctx.conn,
        proj_row_id,
        &task_slug,
        request,
        &branch,
        base_branch.as_deref(),
        base_commit.as_deref(),
    )?;

    // INTAKE-07: the origin is recorded before the Lead agent starts and never cleared.
    if opts.origin.as_deref() == Some(crate::db::ORIGIN_TELEGRAM) {
        crate::db::mark_task_origin(&ctx.conn, task_id, crate::db::ORIGIN_TELEGRAM)?;
    }

    let set =
        crate::db::set_task_workflow_if_null(&ctx.conn, task_id, &resolved.workflow.to_snapshot())?;
    if !set && opts.workflow.is_some() {
        let existing_name = crate::db::task_auto(&ctx.conn, task_id)?
            .and_then(|a| a.workflow_json)
            .and_then(|j| crate::workflows::Workflow::from_snapshot(&j))
            .map(|wf| wf.name)
            .unwrap_or_else(|| "khong-ro".to_string());
        crate::term::emit(&format!(
            "Task đã có quy trình '{existing_name}' — bỏ qua --workflow."
        ));
    }

    // W-1: Lead's Bash git calls go through the guard's git policy; its file
    // writes stay with the allow rules and the post-run backstop.
    let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task_slug, "lead")?;
    let exe = std::env::current_exe()?;
    let settings = crate::guard::write_bash_hook_settings(&run_dir, &proj_path, &exe)?;
    let spec = crate::roles::RoleSpec {
        role: crate::roles::Role::Lead,
        user_input,
        allowed_tools: allowed,
        disallowed_tools: disallowed,
        extra_args: vec![
            "--settings".to_string(),
            settings.to_string_lossy().to_string(),
        ],
    };

    let argv = crate::roles::build_command(&spec, &ctx.cfg)?;
    let model = ctx.cfg.model_for("lead")?;
    let timeout = ctx.cfg.timeout_for("lead");

    let plan = crate::roles::RunPlan {
        role: "lead".to_string(),
        model,
        argv,
        task_id: Some(task_id),
        task_slug: task_slug.clone(),
        base_commit,
        plan_files: None,
        verbose: opts.verbose,
        timeout,
        run_dir: Some(run_dir),
    };

    let round = match opts.round {
        Some(r) => r,
        None => manual_round(&ctx.conn, task_id)?,
    };
    let stage = crate::roles::RunStage {
        stage: Some("plan".to_string()),
        round: Some(round),
        metrics: note.map(|n| serde_json::json!({ "note": n })),
    };
    let role_run = crate::roles::execute_role_staged(ctx, &proj, &proj_path, plan, stage)?;

    match role_run.outcome.status {
        crate::runner::RunStatus::TimedOut => {
            return Err(BrainError::error(format!(
                "Lead chạy quá thời gian (timeout) trên task '{task_slug}'"
            )));
        }
        crate::runner::RunStatus::Killed => {
            return Err(BrainError::new(
                format!("Đã dừng run {} (trạng thái killed)", role_run.run_id),
                crate::error::EXIT_INTERRUPTED,
            ));
        }
        crate::runner::RunStatus::Failed => {
            let code = role_run.outcome.exit_code.unwrap_or(1);
            return Err(BrainError::error(format!(
                "Lead thất bại với mã lỗi {code} trên task '{task_slug}'"
            )));
        }
        _ => {}
    }

    let mut extra = crate::gitops::changed_paths(&proj_path)?;
    extra.remove(".plan.md");
    if !extra.is_empty() {
        crate::term::emit_out("Lead đã sửa file ngoài .plan.md — vi phạm giao thức (ROLE-01):");
        let mut sorted_extra: Vec<String> = extra.into_iter().collect();
        sorted_extra.sort();
        for p in &sorted_extra {
            crate::term::emit_out(&format!("  {p}"));
        }
        crate::term::emit_out(
            "Gợi ý: agent-brain không tự revert thay đổi; bạn hãy kiểm tra và xử lý bằng tay.",
        );
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !plan_path.is_file() {
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Err(BrainError::error(
            "Lead hoàn thành nhưng không tìm thấy file .plan.md",
        ));
    }

    let mut text = std::fs::read_to_string(&plan_path)?;
    let curr_meta = crate::planfile::read_meta(&text);
    if curr_meta.as_ref() != Some(&meta) {
        text = crate::planfile::set_meta(&text, &meta);
        std::fs::write(&plan_path, &text)?;
    }

    let plan = crate::planfile::parse_plan(&text);
    let errors = crate::planfile::validate_plan(&plan, Some(&proj_path));
    if !errors.is_empty() {
        let err_lines = errors
            .iter()
            .map(|e| format!("  - {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        let err_msg = format!(
            "Kế hoạch trong .plan.md không hợp lệ:\n{err_lines}\nSửa tay .plan.md rồi: git -C {} commit -m \"{task_slug}: plan\" -- .plan.md",
            proj_path.display()
        );
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Err(BrainError::error(err_msg));
    }

    let changed = crate::gitops::changed_paths(&proj_path)?;
    if !changed.contains(".plan.md") {
        crate::term::emit_out("Lead không sửa .plan.md — giữ nguyên commit plan hiện tại.");
        crate::db::set_task_status(&ctx.conn, task_id, "planned")?;
        return Ok(crate::error::EXIT_OK);
    }

    crate::gitops::commit_paths(&proj_path, &[".plan.md"], &format!("{task_slug}: plan"))?;
    crate::db::clear_task_approval(&ctx.conn, task_id)?;
    crate::db::set_task_status(&ctx.conn, task_id, "planned")?;
    crate::term::emit_out(&plan_path.to_string_lossy());
    Ok(crate::error::EXIT_OK)
}
