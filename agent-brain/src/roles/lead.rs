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

pub fn cmd_lead(
    ctx: &mut Ctx,
    project: &str,
    request: &str,
    name: Option<&str>,
    _dry_run: bool,
    verbose: bool,
) -> BrainResult<i32> {
    let proj = crate::projects::select_project(project, &ctx.map)?;
    let proj_path = crate::projects::require_runnable(&proj, &ctx.cfg)?;

    let task_slug = crate::routing::slugify(name.unwrap_or(request));
    if task_slug.is_empty() {
        return Err(BrainError::error(
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
        ));
    }
    let branch = format!("task/{task_slug}");

    let mut dirty = crate::gitops::changed_paths(&proj_path)?;
    if crate::gitops::current_branch(&proj_path).as_deref() == Some(&branch) {
        dirty.remove(".plan.md");
    }
    if !dirty.is_empty() {
        let list: Vec<String> = dirty.into_iter().collect();
        return Err(BrainError::error(format!(
            "Working tree có thay đổi chưa commit — dừng, không tự stash:\n{}",
            list.join("\n")
        )));
    }

    let curr_branch = crate::gitops::current_branch(&proj_path);
    let is_replan = curr_branch.as_deref() == Some(&branch);
    let (base_branch, base_commit, created_at) = if is_replan {
        let plan_path = proj_path.join(".plan.md");
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

    let plan_path = proj_path.join(".plan.md");
    let skeleton = crate::planfile::render_plan_skeleton(request, &meta);
    std::fs::write(&plan_path, skeleton)?;

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

    let user_input = format!(
        "Yêu cầu: {}\nTask: {}\nFile .plan.md ở gốc dự án đã có sẵn khung (skeleton). Giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo). Ghi kế hoạch vào .plan.md theo đúng mẫu.",
        request, task_slug
    );

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

    let spec = crate::roles::RoleSpec {
        role: crate::roles::Role::Lead,
        user_input,
        allowed_tools: allowed,
        disallowed_tools: disallowed,
        extra_args: Vec::new(),
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
        verbose,
        timeout,
        run_dir: None,
    };

    let role_run = crate::roles::execute_role(ctx, &proj, &proj_path, plan)?;

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
        for p in &extra {
            crate::term::emit_out(&format!("  {p}"));
        }
        crate::term::emit_out(
            "Gợi ý: agent-brain không tự revert thay đổi; bạn hãy kiểm tra và xử lý bằng tay.",
        );
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !plan_path.is_file() {
        return Err(BrainError::error(
            "Lead hoàn thành nhưng không tìm thấy file .plan.md",
        ));
    }

    let mut text = std::fs::read_to_string(&plan_path)?;
    let curr_meta = crate::planfile::read_meta(&text);
    if curr_meta.as_ref() != Some(&meta) {
        text = crate::planfile::set_meta(&text, &meta);
        std::fs::write(&plan_path, text)?;
    }

    crate::gitops::commit_paths(&proj_path, &[".plan.md"], &format!("{task_slug}: plan"))?;
    crate::db::set_task_status(&ctx.conn, task_id, "planned")?;
    crate::term::emit_out(&plan_path.to_string_lossy());
    Ok(crate::error::EXIT_OK)
}
