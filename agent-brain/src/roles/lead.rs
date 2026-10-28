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
pub const LEAD_AGY_EMPTY_MESSAGE: &str =
    "agy không trả kế hoạch nào — có thể bị từ chối quyền chạy lệnh (xem stderr.txt của run). Chạy lại hoặc đổi engine cho bước plan.";

pub const GIT_CONTEXT_MAX_BYTES: usize = 16 * 1024;

pub fn truncate_git_block(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut cut = max_bytes;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let prefix = &s[..cut];
    if prefix.ends_with('\n') {
        format!("{prefix}… (đã cắt)\n")
    } else {
        format!("{prefix}\n… (đã cắt)\n")
    }
}

pub fn git_context_block(
    branch: Option<&str>,
    status: Option<&str>,
    log: Option<&str>,
) -> String {
    let mut out = String::from(
        "## Ngữ cảnh git (chỉ đọc, agent-brain lấy sẵn — bạn không chạy được lệnh shell)\n",
    );
    if let Some(b) = branch.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("- Nhánh hiện tại: {b}\n"));
    }
    if let Some(st) = status {
        out.push_str("\n### git status --short\n");
        let truncated = truncate_git_block(st, GIT_CONTEXT_MAX_BYTES);
        if truncated.trim().is_empty() {
            out.push_str("(trống)\n");
        } else {
            out.push_str(&truncated);
            if !truncated.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    if let Some(lg) = log {
        out.push_str("\n### git log -n 20 --stat --date=iso\n");
        let truncated = truncate_git_block(lg, GIT_CONTEXT_MAX_BYTES);
        if truncated.trim().is_empty() {
            out.push_str("(trống)\n");
        } else {
            out.push_str(&truncated);
            if !truncated.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// Opening / closing delimiter lines of the forwarded-text block (followed by the nonce and `>>>`).
/// `telegram::compose` writes them, `cmd_lead` looks for them.
pub const UNTRUSTED_OPEN: &str = "<<<TIN_CHUYEN_TIEP ";
pub const UNTRUSTED_CLOSE: &str = "<<<HET_TIN_CHUYEN_TIEP ";

/// INTAKE-06: the Lead gets the untrusted-data rules only for a Telegram-origin task
/// whose request carries the forwarded block.
pub fn wants_untrusted_prompt(origin: Option<&str>, request: &str) -> bool {
    origin == Some(crate::db::ORIGIN_TELEGRAM) && request.contains(UNTRUSTED_OPEN)
}

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
        engine: "claude".to_string(),
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

    // TEAM-03 / D-12: an unknown explicit team is refused here, before any branch or task row.
    let (teams_loaded, _staff) = crate::teams::load();
    let team_res = crate::teams::resolve(
        &teams_loaded,
        opts.team.as_deref(),
        crate::projects::team_of(&ctx.map, &proj.id).as_deref(),
    )?;
    if let Some(ref team_note) = team_res.note {
        crate::term::emit(team_note);
    }
    // ENG-10 / D-36 / D-42: a team naming a profile-restricted engine cannot be bound to a project
    // of another profile (explicit `--team` or the project default). Refused before any branch or
    // task row.
    if let Some(msg) = crate::teams::team_profile_refusal(
        &team_res.team,
        &ctx.cfg.engines,
        &proj.id,
        &proj.profile,
    ) {
        return Err(BrainError::error(msg));
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

    let mut user_input = if keep_plan {
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

    let existing_task_team = crate::db::project_row_id(&ctx.conn, &proj.id)
        .ok()
        .and_then(|pid| {
            ctx.conn
                .query_row(
                    "SELECT team_json FROM tasks WHERE project_id = ?1 AND branch = ?2",
                    rusqlite::params![pid, branch],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
        })
        .and_then(|j| crate::teams::TaskTeam::parse(Some(&j)))
        .and_then(|tt| teams_loaded.get(&tt.name).cloned());
    let effective_team = existing_task_team.as_ref().unwrap_or(&team_res.team);
    let lead_engine = effective_team.engine_for("lead");
    let is_agy_lead = match &lead_engine {
        crate::engine::Engine::Named(id) => {
            ctx.cfg.engines.kind_of(id) == Some(crate::config::EngineKind::Agy)
        }
        _ => false,
    };
    if is_agy_lead {
        let status_res = crate::gitops::git(&proj_path, &["status", "--short"]);
        let log_res = crate::gitops::git(
            &proj_path,
            &["log", "-n", "20", "--stat", "--date=iso"],
        );
        let block = git_context_block(
            curr_branch.as_deref(),
            status_res.as_deref().ok(),
            log_res.as_deref().ok(),
        );
        if !block.is_empty() {
            user_input.push_str("\n\n");
            user_input.push_str(&block);
        }
    }

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
        let untrusted = wants_untrusted_prompt(opts.origin.as_deref(), request);
        let argv = if untrusted {
            crate::roles::build_command_with_prompt(
                &spec_dry,
                &ctx.cfg,
                &crate::roles::lead_untrusted_prompt(),
            )?
        } else {
            crate::roles::build_command(&spec_dry, &ctx.cfg)?
        };
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
            crate::roles::display_argv(
                &argv,
                if untrusted {
                    "assets/prompts/lead.md + lead-untrusted.md"
                } else {
                    "assets/prompts/lead.md"
                }
            )
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

    let skeleton = crate::planfile::render_plan_skeleton(request, &meta);
    let initial_plan_text = if keep_plan {
        let text = std::fs::read_to_string(&plan_path)?;
        let curr_m = crate::planfile::read_meta(&text);
        if curr_m.as_ref() != Some(&meta) {
            let updated = crate::planfile::set_meta(&text, &meta);
            std::fs::write(&plan_path, &updated)?;
            updated
        } else {
            text
        }
    } else {
        std::fs::write(&plan_path, &skeleton)?;
        skeleton.clone()
    };

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

    // The team is recorded before the Lead agent starts and never replaced here.
    let set = crate::db::set_task_team_if_null(
        &ctx.conn,
        task_id,
        &crate::teams::TaskTeam::new(&team_res.team.name).to_json(),
    )?;
    if !set && opts.team.is_some() {
        let existing = crate::db::task_team_json(&ctx.conn, task_id)?
            .and_then(|j| crate::teams::TaskTeam::parse(Some(&j)))
            .map(|t| t.name)
            .unwrap_or_else(|| "khong-ro".to_string());
        crate::term::emit(&format!(
            "Task đã có đội hình '{existing}' — bỏ qua --team."
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

    // INTAKE-06: chosen from the durable origin on every Lead run, so re-plans keep the rules.
    let origin = crate::db::task_origin(&ctx.conn, task_id)?;
    let argv = if wants_untrusted_prompt(origin.as_deref(), request) {
        crate::term::emit("Lead: yêu cầu có tin chuyển tiếp — dùng quy tắc dữ liệu không tin cậy");
        crate::roles::build_command_with_prompt(
            &spec,
            &ctx.cfg,
            &crate::roles::lead_untrusted_prompt(),
        )?
    } else {
        crate::roles::build_command(&spec, &ctx.cfg)?
    };
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

    let engine_kind = crate::db::get_run(&ctx.conn, role_run.run_id)?
        .and_then(|r| ctx.cfg.engines.kind_of(&r.engine));
    let is_agy_run = engine_kind == Some(crate::config::EngineKind::Agy);
    let agy_empty_or_failed = role_run
        .summary
        .agy_response
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
        || role_run.summary.agy_status.as_deref() != Some("SUCCESS");

    let current_plan_text = std::fs::read_to_string(&plan_path).unwrap_or_default();
    let plan_unchanged = current_plan_text == initial_plan_text || current_plan_text == skeleton;

    if is_agy_run && agy_empty_or_failed && plan_unchanged {
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "agy:empty")?;
        crate::term::emit_out(LEAD_AGY_EMPTY_MESSAGE);
        return Err(BrainError::error(LEAD_AGY_EMPTY_MESSAGE));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_context_block_truncates_at_16k_utf8_safely() {
        // Build a string where byte 16384 cuts into the middle of a multibyte character.
        // Crab emoji 🦀 is 4 bytes: [0xF0, 0x9F, 0xA6, 0x80].
        // With 16383 single-byte 'a's followed by 🦀, byte 16384 is inside 🦀.
        let mut text = "a".repeat(GIT_CONTEXT_MAX_BYTES - 1);
        text.push_str("🦀extra content after 16k boundary");

        let out = truncate_git_block(&text, GIT_CONTEXT_MAX_BYTES);
        assert!(out.contains("… (đã cắt)"));
        let cut_part = out.strip_suffix("\n… (đã cắt)\n").unwrap();
        assert!(cut_part.len() <= GIT_CONTEXT_MAX_BYTES);
        assert_eq!(cut_part.len(), GIT_CONTEXT_MAX_BYTES - 1);
        assert!(!cut_part.contains('🦀'));

        // Large status and log: both truncated at 16 KiB
        let large_status = "x".repeat(20_000);
        let large_log = "y".repeat(25_000);
        let block = git_context_block(
            Some("main"),
            Some(large_status.as_str()),
            Some(large_log.as_str()),
        );
        assert!(block.contains("Ngữ cảnh git"));
        assert!(block.contains("- Nhánh hiện tại: main"));
        assert!(block.contains("### git status --short"));
        assert!(block.contains("### git log -n 20 --stat --date=iso"));
        assert_eq!(block.matches("… (đã cắt)").count(), 2);
    }

    #[test]
    fn test_git_context_block_omits_error_blocks() {
        // status is None (simulating git status error), log is Some
        let block_status_err = git_context_block(
            Some("main"),
            None,
            Some("commit 123 - first commit\n"),
        );
        assert!(!block_status_err.contains("### git status --short"));
        assert!(block_status_err.contains("### git log -n 20 --stat --date=iso"));
        assert!(block_status_err.contains("commit 123"));

        // log is None (simulating git log error), status is Some
        let block_log_err = git_context_block(
            Some("main"),
            Some("M file.txt\n"),
            None,
        );
        assert!(block_log_err.contains("### git status --short"));
        assert!(block_log_err.contains("M file.txt"));
        assert!(!block_log_err.contains("### git log -n 20 --stat --date=iso"));

        // both are None (both git commands failed)
        let block_both_err = git_context_block(
            Some("feature-x"),
            None,
            None,
        );
        assert!(block_both_err.contains("- Nhánh hiện tại: feature-x"));
        assert!(!block_both_err.contains("### git status --short"));
        assert!(!block_both_err.contains("### git log -n 20 --stat --date=iso"));
    }

    #[test]
    fn test_git_context_block_empty_output_shows_trong() {
        let block = git_context_block(Some("main"), Some(""), Some(""));
        assert!(block.contains("### git status --short\n(trống)\n"));
        assert!(block.contains("### git log -n 20 --stat --date=iso\n(trống)\n"));
    }
}
