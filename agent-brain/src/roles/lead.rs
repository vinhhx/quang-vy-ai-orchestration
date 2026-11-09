use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

/// Lead's tools without the plan-file rules; `lead_allowed_tools` adds them for the task's path.
pub const LEAD_ALLOWED_TOOLS: &[&str] = crate::with_git_read_only_tools!(
    "Read",
    "Grep",
    "Glob",
    "Bash(git fetch *)",
    "Bash(git fetch)",
);

/// Lead's `--allowedTools` list: `LEAD_ALLOWED_TOOLS` plus Write/Edit of the task's plan file.
pub fn lead_allowed_tools(protocol: &crate::protocol_paths::ProtocolPaths) -> Vec<String> {
    crate::roles::with_protocol_write_rules(LEAD_ALLOWED_TOOLS, &protocol.plan)
}

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
/// C4: the Lead's block on the engine Dev is expected to run on (`engine::preview_for_role`,
/// nothing written), followed by the engine kind's note (`prompts::engine_note`) when one exists.
/// Empty for a task without a team (Dev runs on the default engine and nothing changes).
pub fn dev_engine_block(
    conn: &rusqlite::Connection,
    cfg: &crate::config::Config,
    task_id: i64,
) -> String {
    let resolved = match crate::engine::preview_for_role(conn, Some(task_id), "dev", cfg) {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "## Engine của Dev\nChưa xác định được engine Dev sẽ chạy: {}",
                e.message
            )
        }
    };
    if resolved.team.is_none() {
        return String::new();
    }
    if let crate::engine::Engine::Human(staff_id) = &resolved.engine {
        return format!("## Engine của Dev\nDev dự kiến do người làm ({staff_id}).");
    }
    let Some(kind) = crate::engine::kind_word(cfg, &resolved.engine) else {
        return String::new();
    };
    let who = match &resolved.member {
        Some(pick) => format!("{} · {}", pick.member.display_name, resolved.engine.as_db_str()),
        None => resolved.engine.as_db_str().to_string(),
    };
    let mut block = format!("## Engine của Dev\nDev dự kiến chạy bằng engine {kind} ({who}).");
    if let Some(note) = crate::prompts::engine_note(kind) {
        block.push('\n');
        block.push_str(note.trim_end());
    }
    block
}

/// Also for a bug pulled from Trello / Google Sheet: its request carries the QA data block
/// (`fixbug::QA_DATA_OPEN`), whatever the origin.
pub fn wants_untrusted_prompt(origin: Option<&str>, request: &str) -> bool {
    (origin == Some(crate::db::ORIGIN_TELEGRAM) && request.contains(UNTRUSTED_OPEN))
        || request.contains(crate::fixbug::QA_DATA_OPEN)
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
        error: None,
        finalize: false,
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

    if let Some(ref lvl_str) = opts.level {
        if crate::departments::Level::parse(lvl_str).is_none() {
            return Err(BrainError::error(format!(
                "Mức độ khó '{lvl_str}' không hợp lệ (chỉ nhận 'de', 'vua', 'kho')"
            )));
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
    let protocol = crate::protocol_paths::paths(&proj, &task_slug);

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

    // D2.2 (`workspace: "worktree"`): the task works in its own worktree. A task whose row and
    // branch exist re-plans in its worktree (re-created when it was cleaned up); a new task gets
    // the deterministic worktree path now and the branch + worktree right before the skeleton is
    // written, leaving the main repo's checkout alone. `git_root` is where the repo is read
    // before that worktree exists.
    let repo_root = proj_path;
    let mut wt_new: Option<std::path::PathBuf> = None;
    let proj_path = if proj.workspace.is_worktree() {
        let existing = crate::db::project_row_id(&ctx.conn, &proj.id)
            .ok()
            .and_then(|pid| crate::db::find_task_by_branch(&ctx.conn, pid, &branch).ok().flatten());
        match existing {
            Some(t) if crate::gitops::branch_exists(&repo_root, &branch) => {
                if opts.dry_run {
                    crate::workspace::task_root_at(&repo_root, &t)
                } else {
                    crate::workspace::ensure_task_root(&ctx.conn, &proj, &repo_root, &t)?
                }
            }
            _ if crate::gitops::current_branch(&repo_root).as_deref() == Some(branch.as_str()) => {
                repo_root.clone()
            }
            _ => {
                let p = crate::workspace::worktree_path(&proj.id, &task_slug);
                wt_new = Some(p.clone());
                p
            }
        }
    } else {
        repo_root.clone()
    };
    let git_root = if wt_new.is_some() { repo_root.clone() } else { proj_path.clone() };

    let curr_branch = crate::gitops::current_branch(&git_root);
    let is_replan = wt_new.is_none() && curr_branch.as_deref() == Some(&branch);

    if opts.review {
        if !is_replan {
            return Err(BrainError::error(format!(
                "--review chỉ dùng khi đang ở nhánh {branch}"
            )));
        }
        let review_file = proj_path.join(&protocol.plan_review);
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
            return Err(BrainError::error(format!(
                "--review cần {} có verdict revise (chạy agent-brain run plan-review trước)",
                protocol.plan_review
            )));
        }
    }

    let plan_path = proj_path.join(&protocol.plan);
    let existing_blocker = if is_replan && plan_path.is_file() {
        std::fs::read_to_string(&plan_path)
            .ok()
            .and_then(|t| crate::planfile::blocker_text(&t))
    } else {
        None
    };
    let has_blockers = existing_blocker.is_some();
    let keep_plan =
        is_replan && (opts.review || note.is_some() || has_blockers) && plan_path.is_file();

    let mut user_input = if keep_plan {
        let mut s = format!(
            "Yêu cầu: {}\nTask: {}\nFile {} ở gốc dự án là bản plan hiện tại; sửa trực tiếp trên bản này, giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo).",
            request, task_slug, protocol.plan
        );
        if opts.review {
            s.push_str(&format!(
                "\nBáo cáo review plan: {} (verdict revise). Sửa {} để xử lý mọi mục chặn trong báo cáo.",
                protocol.plan_review, protocol.plan
            ));
        }
        if has_blockers {
            s.push_str("\nPlan có mục Vướng mắc do Dev ghi ở lần chạy trước: giải quyết từng điểm trong plan (bổ sung thông tin, file hoặc khai Thư mục được đọc / Lệnh được chạy), rồi xoá nội dung mục Vướng mắc, chỉ giữ tiêu đề.");
        }
        if let Some(n) = note {
            s.push_str(&format!("\nGhi chú của người dùng: {n}"));
        }
        s
    } else {
        let mut s = format!(
            "Yêu cầu: {}\nTask: {}\nFile {plan} ở gốc dự án đã có sẵn khung (skeleton). Giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo). Ghi kế hoạch vào {plan} theo đúng mẫu.",
            request,
            task_slug,
            plan = protocol.plan
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
    let mut git_block = String::new();
    if is_agy_lead {
        let status_res = crate::gitops::git(&git_root, &["status", "--short"]);
        let log_res = crate::gitops::git(
            &git_root,
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
        git_block = block;
    }

    // Skill mode (D-49): skills are found on every run, no per-project configuration.
    let skill_block = crate::skills::prompt_block(&crate::skills::discover(&git_root));
    if !skill_block.is_empty() {
        user_input.push_str("\n\n");
        user_input.push_str(&skill_block);
    }

    let abs_plan_rules =
        crate::roles::abs_rules(&proj_path, &[protocol.plan.as_str()], &["Write", "Edit"]);
    let mut allowed = lead_allowed_tools(&protocol);
    allowed.extend(abs_plan_rules);

    let disallowed = LEAD_DISALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let (sources, _) = crate::projects::read_source_paths(&ctx.map, &proj);
    let source_paths: Vec<std::path::PathBuf> = sources.into_iter().map(|(_, p)| p).collect();

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
            source_paths: source_paths.clone(),
        };
        let layers = crate::prompts::PromptLayers {
            stacks: proj.stack.clone(),
            project: Some(proj.id.clone()),
            workflow: Some(resolved.workflow.name.clone()),
            team: Some(effective_team.name.clone()),
        };
        let untrusted = wants_untrusted_prompt(opts.origin.as_deref(), request);
        let base_prompt = if untrusted {
            crate::roles::lead_untrusted_prompt()
        } else {
            crate::roles::prompt_for(crate::roles::Role::Lead).to_string()
        };
        let (argv, warnings) = crate::roles::build_command_layered(
            &spec_dry,
            &ctx.cfg,
            &base_prompt,
            &layers,
        )?;
        for w in warnings {
            crate::term::emit(&w);
        }
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
        crate::term::emit_out(&crate::prompts::dry_run_summary(&layers, "lead"));
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

    // A new task in worktree mode starts from a fresh worktree: the main repo's local changes
    // are not part of it.
    let mut dirty = if wt_new.is_some() {
        Default::default()
    } else {
        crate::gitops::changed_paths(&proj_path)?
    };
    if crate::gitops::current_branch(&git_root).as_deref() == Some(&branch) {
        dirty.remove(&protocol.plan);
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
    } else if crate::gitops::branch_exists(&git_root, &branch) {
        return Err(BrainError::error(format!(
            "Nhánh '{branch}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác"
        )));
    } else {
        // D2.1: the project's `base_branch` when set (it must exist), else the current branch.
        let (bb, bc) = match proj.workspace.base_branch.as_deref() {
            Some(base) => {
                if !crate::gitops::branch_exists(&git_root, base) {
                    return Err(BrainError::error(crate::projects::base_branch_missing_text(
                        base,
                    )));
                }
                let head_ref = format!("refs/heads/{base}");
                let sha = crate::gitops::git(&git_root, &["rev-parse", "--verify", &head_ref])?
                    .trim()
                    .to_string();
                (Some(base.to_string()), sha)
            }
            None => (curr_branch.clone(), crate::gitops::head_sha(&git_root)?),
        };
        let ca = chrono::Utc::now().to_rfc3339();
        if let Some(ref cb) = bb {
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

    if let Some(ref wt) = wt_new {
        // D2.2: the branch from the base (the configured one, else the base commit read above:
        // the main repo's HEAD) and its worktree; the main repo keeps its checkout.
        let start = proj
            .workspace
            .base_branch
            .clone()
            .or_else(|| base_commit.clone())
            .unwrap_or_else(|| "HEAD".to_string());
        let made =
            crate::workspace::create_task_worktree(&repo_root, &proj.id, &task_slug, &branch, &start)?;
        if &made != wt {
            return Err(BrainError::error(format!(
                "Worktree tạo ở {} thay vì {}",
                made.display(),
                wt.display()
            )));
        }
    } else if !is_replan {
        // A configured base branch is the start point; otherwise HEAD (the current branch).
        match proj.workspace.base_branch.as_deref() {
            Some(base) => crate::gitops::create_branch_from(&proj_path, &branch, base)?,
            None => crate::gitops::create_branch(&proj_path, &branch)?,
        }
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
        protocol.ensure_dir(&proj_path)?;
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

    if proj.workspace.is_worktree() && proj_path != repo_root {
        crate::db::set_task_workdir(&ctx.conn, task_id, Some(&proj_path.to_string_lossy()))?;
    }

    if let Some(ref lvl_str) = opts.level {
        let lvl = crate::departments::Level::parse(lvl_str).unwrap();
        crate::db::set_task_level(&ctx.conn, task_id, lvl.as_str())?;
    }

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
    let origin = crate::db::task_origin(&ctx.conn, task_id)?;
    let round = match opts.round {
        Some(r) => r,
        None => manual_round(&ctx.conn, task_id)?,
    };
    let model = ctx.cfg.model_for("lead")?;
    let task_wf = crate::roles::task_workflow(&ctx.conn, Some(task_id));
    let timeout = crate::roles::task_timeout(
        ctx,
        &proj.id,
        Some(task_wf.as_ref().unwrap_or(&resolved.workflow)),
        "lead",
    );
    let exe = std::env::current_exe()?;

    let mut current_input = user_input;
    let mut current_initial_plan = initial_plan_text;

    for attempt in 1..=2 {
        let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task_slug, "lead")?;
        let settings = crate::guard::write_bash_hook_settings(&run_dir, &proj_path, &exe)?;
        let spec = crate::roles::RoleSpec {
            role: crate::roles::Role::Lead,
            user_input: current_input.clone(),
            allowed_tools: allowed.clone(),
            disallowed_tools: disallowed.clone(),
            extra_args: vec![
                "--settings".to_string(),
                settings.to_string_lossy().to_string(),
            ],
            source_paths: source_paths.clone(),
        };

        let layers = crate::prompts::PromptLayers {
            stacks: proj.stack.clone(),
            project: Some(proj.id.clone()),
            workflow: Some(resolved.workflow.name.clone()),
            team: Some(effective_team.name.clone()),
        };
        let base_prompt = if wants_untrusted_prompt(origin.as_deref(), request) {
            crate::term::emit("Lead: yêu cầu có tin chuyển tiếp — dùng quy tắc dữ liệu không tin cậy");
            crate::roles::lead_untrusted_prompt()
        } else {
            crate::roles::prompt_for(crate::roles::Role::Lead).to_string()
        };

        let (argv, warnings) = crate::roles::build_command_layered(
            &spec,
            &ctx.cfg,
            &base_prompt,
            &layers,
        )?;
        for w in warnings {
            crate::term::emit(&w);
        }
        let mut argv = argv;
        crate::engine::append_to_system_prompt(
            &mut argv,
            &dev_engine_block(&ctx.conn, &ctx.cfg, task_id),
        );

        let plan = crate::roles::RunPlan {
            role: "lead".to_string(),
            model: model.clone(),
            argv,
            task_id: Some(task_id),
            task_slug: task_slug.clone(),
            base_commit: base_commit.clone(),
            plan_files: None,
            verbose: opts.verbose,
            timeout,
            run_dir: Some(run_dir),
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
        let plan_unchanged =
            current_plan_text == current_initial_plan || current_plan_text == skeleton;

        if is_agy_run && agy_empty_or_failed && plan_unchanged {
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "agy:empty")?;
            crate::term::emit_out(LEAD_AGY_EMPTY_MESSAGE);
            return Err(BrainError::error(LEAD_AGY_EMPTY_MESSAGE));
        }

        let mut extra = crate::gitops::changed_paths(&proj_path)?;
        extra.remove(&protocol.plan);
        if !extra.is_empty() {
            crate::term::emit_out(&format!(
                "Lead đã sửa file ngoài {} — vi phạm giao thức (ROLE-01):",
                protocol.plan
            ));
            let mut sorted_extra: Vec<String> = extra.into_iter().collect();
            sorted_extra.sort();
            for p in &sorted_extra {
                crate::term::emit_out(&format!("  {p}"));
            }
            crate::term::emit_out(
                "Gợi ý: agent-brain không tự revert thay đổi; bạn hãy kiểm tra và xử lý bằng tay.",
            );
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
            let total = sorted_extra.len();
            let files_50: Vec<String> = sorted_extra.into_iter().take(50).collect();
            let preview = if total > files_50.len() {
                format!("{}, … (và {} file khác)", files_50.join(", "), total - files_50.len())
            } else {
                files_50.join(", ")
            };
            let msg = format!(
                "{total} file ngoài {plan}: {preview} — Lead chỉ được sửa {plan}",
                plan = protocol.plan
            );
            crate::roles::record_violation(
                ctx,
                role_run.run_id,
                "files_outside_plan",
                &msg,
                serde_json::json!({ "files": files_50, "total": total }),
            )?;
            return Ok(crate::error::EXIT_PROTOCOL);
        }

        if !plan_path.is_file() {
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
            let msg = format!("Lead hoàn thành nhưng không tìm thấy file {}", protocol.plan);
            crate::roles::record_violation(
                ctx,
                role_run.run_id,
                "missing_plan",
                &msg,
                serde_json::json!({}),
            )?;
            return Err(BrainError::error(msg));
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
            if attempt == 1 {
                crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "plan_invalid")?;
                let msg = format!("Plan không hợp lệ (lần 1/2): {}", errors.join("; "));
                crate::roles::record_violation(
                    ctx,
                    role_run.run_id,
                    "plan_invalid",
                    &msg,
                    serde_json::json!({ "attempt": 1, "errors": errors }),
                )?;
                crate::term::emit_out("Plan không hợp lệ — Lead tự sửa lại (lần 2/2):");
                for e in &errors {
                    crate::term::emit_out(&format!("  - {e}"));
                }
                let err_lines = errors
                    .iter()
                    .map(|e| format!("  - {e}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut retry_input = format!(
                    "Yêu cầu: {request}\nTask: {task_slug}\nFile {plan} ở gốc dự án là bản plan hiện tại; sửa trực tiếp trên bản này, giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo).\nLần chạy trước viết {plan} không hợp lệ. Lỗi:\n{err_lines}\nChỉ sửa {plan} để hết các lỗi trên: bỏ hoặc thay lệnh không được phép bằng lệnh hợp lệ, giữ nguyên phần còn lại của plan.",
                    plan = protocol.plan
                );
                if let Some(n) = note {
                    retry_input.push_str(&format!("\nGhi chú của người dùng: {n}"));
                }
                if is_agy_lead && !git_block.is_empty() {
                    retry_input.push_str("\n\n");
                    retry_input.push_str(&git_block);
                }
                if !skill_block.is_empty() {
                    retry_input.push_str("\n\n");
                    retry_input.push_str(&skill_block);
                }
                current_input = retry_input;
                current_initial_plan = std::fs::read_to_string(&plan_path).unwrap_or_default();
                continue;
            } else {
                let err_lines = errors
                    .iter()
                    .map(|e| format!("  - {e}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let err_msg = format!(
                    "Lead đã tự sửa 1 lần nhưng plan vẫn không hợp lệ. Kế hoạch trong {plan} không hợp lệ:\n{err_lines}\nSửa tay {plan} rồi: git -C {} commit -m \"{task_slug}: plan\" -- {plan}",
                    proj_path.display(),
                    plan = protocol.plan
                );
                crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
                let short_msg = format!(
                    "Plan không hợp lệ sau khi Lead tự sửa (lần 2/2): {}",
                    errors.join("; ")
                );
                crate::roles::record_violation(
                    ctx,
                    role_run.run_id,
                    "plan_invalid",
                    &short_msg,
                    serde_json::json!({ "attempt": 2, "errors": errors }),
                )?;
                return Err(BrainError::error(err_msg));
            }
        }

        break;
    }

    let changed = crate::gitops::changed_paths(&proj_path)?;
    if !changed.contains(&protocol.plan) {
        crate::term::emit_out(&format!(
            "Lead không sửa {} — giữ nguyên commit plan hiện tại.",
            protocol.plan
        ));
        crate::db::set_task_status(&ctx.conn, task_id, "planned")?;
        return Ok(crate::error::EXIT_OK);
    }

    crate::gitops::commit_paths(
        &proj_path,
        &[protocol.plan.as_str()],
        &format!("{task_slug}: plan"),
    )?;
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
