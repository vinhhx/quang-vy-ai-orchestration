use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

pub const DEV_DISALLOWED_TOOLS: [&str; 6] = [
    "Grep",
    "Glob",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Task",
];

/// Printed when a successful agy Dev run changed nothing (outcome `agy:empty`).
pub const AGY_EMPTY_MESSAGE: &str =
    "agy không sửa file nào — có thể bị từ chối quyền (xem stderr.txt của run).";

/// Printed when a successful dsh Dev run changed nothing (outcome `dsh:empty`).
pub const DSH_EMPTY_MESSAGE: &str = "dsh không sửa file nào trong lượt này — có thể container không ghi được vào dự án hoặc dsh chỉ trả lời. Xem log rồi chạy lại hoặc đổi đội hình.";

/// Context file a previous engine may leave for the next one (Phase 4); read-only for Dev.
const HANDOVER_FILE: &str = ".handover.md";

/// ENG-03 / D-18: true when the latest Dev run of the task on the current branch ended with
/// outcome `quota`. Read-only — never creates a task row. The old session is never resumed.
fn quota_continuation(
    conn: &rusqlite::Connection,
    project_slug: &str,
    root: &Path,
) -> BrainResult<bool> {
    let Some(branch) = crate::gitops::current_branch(root) else {
        return Ok(false);
    };
    let Ok(project_row) = crate::db::project_row_id(conn, project_slug) else {
        return Ok(false);
    };
    let Some(task) = crate::db::find_task_by_branch(conn, project_row, &branch)? else {
        return Ok(false);
    };
    let latest = crate::db::latest_run_for_task_role(conn, task.id, "dev")?;
    Ok(latest.and_then(|r| r.outcome).as_deref() == Some(crate::quota::OUTCOME_QUOTA))
}

pub fn dev_allowed_tools(files: &[String], root: Option<&Path>) -> Vec<String> {
    let mut all_files = files.to_vec();
    if !all_files
        .iter()
        .any(|f| f.strip_prefix("./").unwrap_or(f) == ".plan.md")
    {
        all_files.push(".plan.md".to_string());
    }
    let mut tools = Vec::new();
    for f in &all_files {
        let clean_f = f.strip_prefix("./").unwrap_or(f);
        tools.push(format!("Read(./{clean_f})"));
        tools.push(format!("Edit(./{clean_f})"));
        tools.push(format!("Write(./{clean_f})"));
        if let Some(r) = root {
            tools.extend(crate::roles::abs_rules(
                r,
                &[clean_f],
                &["Read", "Edit", "Write"],
            ));
        }
    }
    tools.push("Bash(git add *)".to_string());
    tools.push("Bash(git commit *)".to_string());
    tools
}

pub fn cmd_dev(ctx: &mut Ctx, project: &str, opts: &crate::roles::RunOpts) -> BrainResult<i32> {
    let dry_run = opts.dry_run;
    let verbose = opts.verbose;
    let proj = crate::projects::select_project(project, &ctx.map)?;
    let proj_path = crate::projects::require_runnable(&proj, &ctx.cfg)?;

    let plan_path = proj_path.join(".plan.md");
    if !plan_path.is_file() {
        return Err(BrainError::error("Không tìm thấy .plan.md ở gốc dự án"));
    }

    let text = std::fs::read_to_string(&plan_path)?;
    let meta = crate::planfile::read_meta(&text);
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

    // ENG-03 / D-18: the latest Dev run of this task stopped on quota, so the partial diff it left
    // is the work to continue, not something to refuse. Read-only: no task row is created.
    let continuation = quota_continuation(&ctx.conn, &proj.id, &proj_path)?;
    let plan = crate::planfile::parse_plan(&text);

    let dirty = crate::gitops::changed_paths(&proj_path)?;
    if !dirty.is_empty() {
        let mut sorted_dirty: Vec<String> = if continuation {
            // Only the plan's files, protocol files and `.handover.md` may be dirty; anything
            // else is still refused with the same message.
            let plan_set: std::collections::BTreeSet<String> = crate::planfile::plan_files(&plan)
                .iter()
                .map(|f| crate::planfile::normalize_plan_path(f).nfc().collect())
                .collect();
            dirty
                .into_iter()
                .filter(|p| {
                    let n: String = p.nfc().collect();
                    !(plan_set.contains(&n)
                        || crate::stream::PROTOCOL_FILES.contains(&n.as_str())
                        || n == HANDOVER_FILE)
                })
                .collect()
        } else {
            dirty.into_iter().collect()
        };
        sorted_dirty.sort();
        if !sorted_dirty.is_empty() {
            let list = sorted_dirty
                .iter()
                .map(|p| format!("  {p}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(BrainError::error(format!(
                "Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message '{task}: plan':\n{list}"
            )));
        }
    }

    let errors = crate::planfile::validate_plan(&plan, Some(&proj_path));
    let files = crate::planfile::plan_files(&plan);
    if !errors.is_empty() || files.is_empty() {
        let err_msg = if !errors.is_empty() {
            let err_list = errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("Kế hoạch trong .plan.md không hợp lệ:\n{err_list}")
        } else {
            "Kế hoạch không có file nào trong bảng Thay đổi".to_string()
        };
        return Err(BrainError::error(err_msg));
    }

    if crate::planfile::has_blockers(&plan) {
        return Err(BrainError::error(
            "Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev",
        ));
    }

    let mut named_list: Vec<String> = Vec::new();
    let (active_files, mut allowed, mut user_input, prompt_path, is_fix) = if let Some(fix) =
        opts.fix
    {
        let report_file = fix.report_file();
        let report_path = proj_path.join(report_file);
        if !report_path.is_file() {
            let action = match fix {
                crate::roles::FixInput::CodeReview => "review code",
                crate::roles::FixInput::Qa => "QA",
            };
            return Err(BrainError::error(format!(
                "Không tìm thấy {report_file} — chạy {action} trước khi chạy chế độ sửa"
            )));
        }

        let report_text = std::fs::read_to_string(&report_path)?;
        match fix {
            crate::roles::FixInput::CodeReview => {
                let r = crate::report::parse_review(&report_text, crate::report::ReviewKind::Code)
                    .map_err(|reason| {
                        BrainError::error(format!(
                            "Báo cáo .code-review.md không đúng định dạng: {reason}"
                        ))
                    })?;
                if r.verdict != "revise" {
                    return Err(BrainError::error(format!(
                        "Báo cáo .code-review.md không yêu cầu sửa (verdict {}) — không chạy chế độ sửa",
                        r.verdict
                    )));
                }
            }
            crate::roles::FixInput::Qa => {
                let verdict = crate::planfile::parse_qa_verdict(&report_text);
                if verdict.as_deref() != Some("Cần sửa") {
                    return Err(BrainError::error(
                        "Báo cáo .qa-report.md không kết luận Cần sửa — không chạy chế độ sửa",
                    ));
                }
            }
        }

        // WR-06: body tokens only widen the write set for files already changed
        // since the plan commit; front-matter `files` stay the explicit way (D-12).
        let changed = match crate::roles::plan_commit_for(&ctx.conn, &proj.id, &proj_path, &task)? {
            Some(pc) => crate::gitops::diff_code_names(&proj_path, &pc, "HEAD")?,
            None => std::collections::BTreeSet::new(),
        };
        let named = crate::report::named_files_with(&report_text, &proj_path, Some(&changed));
        if named.files.is_empty() {
            crate::term::emit_out(&format!("File được nêu trong {report_file}: (không có)"));
        } else {
            crate::term::emit_out(&format!(
                "File được nêu trong {report_file}: {}",
                named.files.join(", ")
            ));
        }
        for (path, code) in &named.rejected {
            crate::term::emit_out(&format!("Bỏ qua (không được phép): {path} ({code})"));
        }

        let mut fix_files = files.clone();
        for nf in &named.files {
            let norm_nf = crate::planfile::normalize_plan_path(nf);
            if !fix_files
                .iter()
                .any(|f| crate::planfile::normalize_plan_path(f) == norm_nf)
            {
                fix_files.push(nf.clone());
            }
        }

        named_list = named.files.clone();

        let mut allowed = dev_allowed_tools(&fix_files, Some(&proj_path));
        allowed.push(format!("Read(./{report_file})"));
        allowed.extend(crate::roles::abs_rules(
            &proj_path,
            &[report_file],
            &["Read"],
        ));

        let report_desc = match fix {
            crate::roles::FixInput::CodeReview => "báo cáo review code",
            crate::roles::FixInput::Qa => "báo cáo QA",
        };
        let user_input = format!(
            "Task: {task}\nChế độ sửa theo {report_desc} ({report_file}).\nĐọc .plan.md và {report_file}, sửa mọi mục chặn và lỗi được nêu, không làm thêm việc khác.\nFile được đọc và sửa: {}\nCommit với message: \"{task}: sửa theo {}\".",
            fix_files.join(", "),
            fix.label()
        );

        (
            fix_files,
            allowed,
            user_input,
            "assets/prompts/dev-fix.md",
            true,
        )
    } else {
        let allowed = dev_allowed_tools(&files, Some(&proj_path));
        let user_input = format!(
            "Task: {task}\nĐọc .plan.md rồi sửa đúng các mục trong bảng Thay đổi.\nFile được đọc và sửa: {}\nCommit với message: \"{task}: <mô tả ngắn>\".",
            files.join(", ")
        );
        (
            files.clone(),
            allowed,
            user_input,
            "assets/prompts/dev.md",
            false,
        )
    };

    // `.handover.md` (Phase 4) is included only when the file exists.
    let handover_exists = continuation && proj_path.join(HANDOVER_FILE).is_file();
    if continuation {
        let handover = if handover_exists {
            ", đọc .handover.md"
        } else {
            ""
        };
        user_input.push_str(&format!(
            "\n\nTiếp tục việc dở của lượt trước (dừng vì hết quota): đọc .plan.md, xem thay đổi hiện có bằng git diff và git status{handover}; làm tiếp phần còn thiếu, không làm lại từ đầu và không hoàn tác thay đổi đã có."
        ));
        allowed.push("Bash(git diff *)".to_string());
        allowed.push("Bash(git status *)".to_string());
        if handover_exists {
            allowed.push(format!("Read(./{HANDOVER_FILE})"));
            allowed.extend(crate::roles::abs_rules(
                &proj_path,
                &[HANDOVER_FILE],
                &["Read"],
            ));
        }
    }

    let disallowed = DEV_DISALLOWED_TOOLS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if dry_run {
        // D-19: a human Dev step runs no command. Read-only lookup, never creates a task row.
        let existing_task_id = crate::db::project_row_id(&ctx.conn, &proj.id)
            .ok()
            .and_then(|pid| {
                crate::gitops::current_branch(&proj_path).and_then(|b| {
                    crate::db::find_task_by_branch(&ctx.conn, pid, &b)
                        .ok()
                        .flatten()
                })
            })
            .map(|t| t.id);
        if let Ok(r) = crate::engine::resolve_for_run(&ctx.conn, existing_task_id, "dev") {
            if r.engine.is_human() {
                crate::term::emit_out("Bước Dev do người làm — không chạy lệnh nào (dry run).");
                return Ok(crate::error::EXIT_OK);
            }
        }
        crate::term::emit_out(&format!("Allowlist: {}", active_files.join(", ")));
        let spec_dry = crate::roles::RoleSpec {
            role: crate::roles::Role::Dev,
            user_input,
            allowed_tools: allowed,
            disallowed_tools: disallowed,
            extra_args: vec![
                "--settings".to_string(),
                "<run_dir/settings.json>".to_string(),
            ],
        };
        let cmd = if is_fix {
            crate::roles::build_command_with_prompt(
                &spec_dry,
                &ctx.cfg,
                crate::roles::DEV_FIX_PROMPT,
            )?
        } else {
            crate::roles::build_command(&spec_dry, &ctx.cfg)?
        };
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&cmd, prompt_path)
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_row = crate::tasks::current_task(&ctx.conn, proj_row_id, &proj_path)?;
    let task_id = task_row.as_ref().map(|t| t.id);

    if opts.fix.is_none() {
        if let Some(tid) = task_id {
            if let Some(auto) = crate::db::task_auto(&ctx.conn, tid)? {
                if auto.workflow_json.is_some() {
                    if let Some(pc) =
                        crate::roles::plan_commit_for(&ctx.conn, &proj.id, &proj_path, &task)?
                    {
                        if auto.approved_commit.as_deref() != Some(pc.as_str()) {
                            crate::db::set_task_approval(&ctx.conn, tid, crate::db::now_ms(), &pc)?;
                            crate::term::emit(&format!(
                                "Ghi nhận duyệt plan {} — chạy Dev trực tiếp cũng coi là đã duyệt",
                                &pc[..7.min(pc.len())]
                            ));
                        }
                    }
                }
            }
        }
    }

    // D-19 / D-24: when the task's Dev engine is a person, every precondition above has passed;
    // nothing is spawned and no run row is inserted — the step becomes a wait marker.
    let resolved = crate::engine::resolve_for_run(&ctx.conn, task_id, "dev")?;
    if let crate::engine::Engine::Human(staff_id) = &resolved.engine {
        let Some(row) = task_row.as_ref() else {
            return Err(BrainError::error(
                "Bước Dev do người làm cần task đã có trong brain.db — chạy run lead trước",
            ));
        };
        let (human_stage, fix_for) = match opts.fix {
            Some(fix) => ("fix", Some(fix.metrics_value())),
            None => ("implement", None),
        };
        return crate::human::start_wait(
            ctx,
            &proj,
            &proj_path,
            row,
            staff_id,
            human_stage,
            fix_for,
            opts.round,
        );
    }

    let dev_base = crate::gitops::head_sha(&proj_path)?;
    let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task, "dev")?;

    let exe = std::env::current_exe()?;
    let settings = if let Some(fix) = opts.fix {
        let mut write_allow = Vec::new();
        for f in &active_files {
            write_allow.push(proj_path.join(f));
        }
        write_allow.push(proj_path.join(".plan.md"));

        let mut read_allow = write_allow.clone();
        read_allow.push(proj_path.join(fix.report_file()));
        if handover_exists {
            read_allow.push(proj_path.join(HANDOVER_FILE));
        }

        crate::guard::write_hook_settings(
            &run_dir,
            &proj_path,
            Some(&read_allow),
            &write_allow,
            &exe,
        )?
    } else {
        let mut allow_paths = Vec::new();
        for f in &active_files {
            allow_paths.push(proj_path.join(f));
        }
        allow_paths.push(proj_path.join(".plan.md"));

        let mut read_allow = allow_paths.clone();
        if handover_exists {
            read_allow.push(proj_path.join(HANDOVER_FILE));
        }
        crate::guard::write_hook_settings(
            &run_dir,
            &proj_path,
            Some(&read_allow),
            &allow_paths,
            &exe,
        )?
    };

    let spec = crate::roles::RoleSpec {
        role: crate::roles::Role::Dev,
        user_input,
        allowed_tools: allowed,
        disallowed_tools: disallowed,
        extra_args: vec![
            "--settings".to_string(),
            settings.to_string_lossy().to_string(),
        ],
    };

    let argv = if is_fix {
        crate::roles::build_command_with_prompt(&spec, &ctx.cfg, crate::roles::DEV_FIX_PROMPT)?
    } else {
        crate::roles::build_command(&spec, &ctx.cfg)?
    };
    let model = ctx.cfg.model_for("dev")?;
    let timeout = ctx.cfg.timeout_for("dev");

    let run_plan = crate::roles::RunPlan {
        role: "dev".to_string(),
        model,
        argv,
        task_id,
        task_slug: task.clone(),
        base_commit: Some(dev_base.clone()),
        plan_files: Some(active_files.clone()),
        verbose,
        timeout,
        run_dir: Some(run_dir),
    };

    let stage = if let Some(fix) = opts.fix {
        let mut metrics = serde_json::json!({
            "fix_for": fix.metrics_value(),
            "named_files": named_list,
        });
        if continuation {
            metrics["continuation"] = serde_json::Value::Bool(true);
        }
        crate::roles::RunStage {
            stage: Some("fix".to_string()),
            round: opts.round,
            metrics: Some(metrics),
        }
    } else {
        crate::roles::RunStage {
            stage: Some("implement".to_string()),
            round: opts.round,
            metrics: continuation.then(|| serde_json::json!({ "continuation": true })),
        }
    };

    let role_run = crate::roles::execute_role_staged(ctx, &proj, &proj_path, run_plan, stage)?;

    let changed_now = crate::gitops::changed_paths(&proj_path)?;
    let emit_changed_now = |changed: &std::collections::BTreeSet<String>| {
        if !changed.is_empty() {
            let list: Vec<String> = changed.iter().cloned().collect();
            crate::term::emit(&format!(
                "Các file đã thay đổi:\n{}",
                list.iter()
                    .map(|p| format!("  {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    };

    match role_run.outcome.status {
        crate::runner::RunStatus::TimedOut => {
            emit_changed_now(&changed_now);
            return Err(BrainError::error(format!(
                "Dev chạy quá thời gian (timeout) trên task '{task}'"
            )));
        }
        crate::runner::RunStatus::Killed => {
            emit_changed_now(&changed_now);
            return Err(BrainError::new(
                "Dev bị ngắt bởi người dùng",
                crate::error::EXIT_INTERRUPTED,
            ));
        }
        crate::runner::RunStatus::Failed => {
            emit_changed_now(&changed_now);
            let code = role_run.outcome.exit_code.unwrap_or(1);
            return Err(BrainError::error(format!(
                "Dev thất bại với mã lỗi {code} trên task '{task}'"
            )));
        }
        _ => {
            if let Some(code) = role_run.outcome.exit_code {
                if code != 0 {
                    emit_changed_now(&changed_now);
                    return Err(BrainError::error(format!(
                        "Dev thất bại với mã lỗi {code} trên task '{task}'"
                    )));
                }
            }
        }
    }

    let diff_tree = crate::gitops::diff_names(&proj_path, &dev_base, "HEAD")?;
    let uncommitted = crate::gitops::changed_paths(&proj_path)?;
    let mut all_changed = diff_tree;
    all_changed.extend(uncommitted);

    let mut all_changed_nfc: std::collections::BTreeSet<String> =
        all_changed.into_iter().map(|p| p.nfc().collect()).collect();
    if handover_exists {
        // Read-only context left for the continuation; neither a scope violation nor a deliverable.
        all_changed_nfc.remove(HANDOVER_FILE);
    }

    let mut allowed_nfc: std::collections::BTreeSet<String> = active_files
        .iter()
        .map(|f| {
            let norm = crate::planfile::normalize_plan_path(f);
            norm.nfc().collect()
        })
        .collect();
    allowed_nfc.insert(".plan.md".to_string());

    // D-05 / D-35: an agy Dev edits with `--mode accept-edits` and a dsh Dev writes through the
    // container, so their diffs get two extra checks. They key on the engine KIND of the run's
    // engine id, so every agy-kind and dsh-kind entry is covered; an unknown id gets neither.
    let engine_kind = crate::db::get_run(&ctx.conn, role_run.run_id)?
        .and_then(|r| ctx.cfg.engines.kind_of(&r.engine));
    let empty_outcome: Option<(&str, &str)> = match engine_kind {
        Some(crate::config::EngineKind::Agy) => Some(("agy:empty", AGY_EMPTY_MESSAGE)),
        Some(crate::config::EngineKind::Dsh) => Some(("dsh:empty", DSH_EMPTY_MESSAGE)),
        _ => None,
    };

    let mut violations: Vec<String> = all_changed_nfc.difference(&allowed_nfc).cloned().collect();
    if empty_outcome.is_some() {
        // `.planning/` and secret-looking paths are never agy's or dsh's to change, even when
        // the plan lists them.
        for p in &all_changed_nfc {
            if (p.starts_with(".planning/") || crate::report::sensitive_path(p))
                && !violations.contains(p)
            {
                violations.push(p.clone());
            }
        }
    }
    if !violations.is_empty() {
        crate::term::emit_out("Dev đã sửa file ngoài plan — vi phạm giao thức (ROLE-06):");
        let mut sorted_violations = violations;
        sorted_violations.sort();
        for p in &sorted_violations {
            crate::term::emit_out(&format!("  {p}"));
        }
        crate::term::emit_out("agent-brain không tự hoàn tác");
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    // T-06-33 / T-06-94: agy answers SUCCESS even when its tools were soft-denied (S7b), and dsh
    // may only answer in text. An empty diff after a successful agy or dsh Dev run is therefore
    // suspicious, not done.
    if let Some((outcome, message)) = empty_outcome {
        if all_changed_nfc.is_empty() {
            crate::term::emit_out(message);
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, outcome)?;
            return Ok(crate::error::EXIT_BLOCKED);
        }
    }

    let plan_curr_text = std::fs::read_to_string(&plan_path)?;
    let plan_curr = crate::planfile::parse_plan(&plan_curr_text);
    let has_block = crate::planfile::has_blockers(&plan_curr);

    let mut pending = crate::gitops::changed_paths(&proj_path)?;
    if handover_exists {
        pending.remove(HANDOVER_FILE);
    }
    if !pending.is_empty() {
        let msg = if has_block {
            format!("{task}: vướng mắc")
        } else {
            let muctieu = plan_curr
                .sections
                .get("Mục tiêu")
                .map(|s| s.as_str())
                .unwrap_or("");
            static PREFIX_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            let prefix_re = PREFIX_RE.get_or_init(|| regex::Regex::new(r"^[-*+\d.]+\s*").unwrap());
            let mut short = String::new();
            for line in muctieu.lines() {
                let trimmed = line.trim();
                let stripped = prefix_re.replace(trimmed, "").trim().to_string();
                if !stripped.is_empty() {
                    short = stripped.chars().take(72).collect();
                    break;
                }
            }
            if short.is_empty() {
                short = "dev".to_string();
            }
            format!("{task}: {short}")
        };
        let mut sorted_pending: Vec<String> = pending.into_iter().collect();
        sorted_pending.sort();
        let pending_refs: Vec<&str> = sorted_pending.iter().map(|s| s.as_str()).collect();
        crate::gitops::commit_paths(&proj_path, &pending_refs, &msg)?;
    }

    let commits = crate::gitops::commit_subjects(&proj_path, &dev_base, "HEAD")?;
    for (sha, subj) in commits {
        if !subj.starts_with(&format!("{task}: ")) {
            let sha_short = if sha.len() >= 7 { &sha[..7] } else { &sha };
            crate::term::emit(&format!(
                "CẢNH BÁO: Commit {sha_short} có subject không đúng chuẩn '{task}: ...': '{subj}'"
            ));
        }
    }

    let final_changed = crate::gitops::diff_names(&proj_path, &dev_base, "HEAD")?;
    if !final_changed.is_empty() {
        crate::term::emit_out("Các file đã sửa:");
        let mut sorted_final: Vec<String> = final_changed.into_iter().collect();
        sorted_final.sort();
        for p in sorted_final {
            crate::term::emit_out(&format!("  {p}"));
        }
    }

    let kt_text = crate::planfile::section_text(&plan_curr_text, "Cách kiểm tra");
    crate::term::emit_out(&format!(
        "Cách kiểm tra (agent-brain không tự chạy — hãy chạy tay trong {}):\n{kt_text}",
        proj_path.display()
    ));

    if has_block {
        let vm_text = crate::planfile::section_text(&plan_curr_text, "Vướng mắc");
        crate::term::emit_out(&format!("Vướng mắc:\n{vm_text}"));
        crate::term::emit_out("Bước tiếp theo: sửa .plan.md hoặc chạy lại agent-brain run lead");
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "dev:blocked")?;
        if let Some(tid) = task_id {
            crate::db::set_task_status(&ctx.conn, tid, "blocked")?;
        }
        return Ok(crate::error::EXIT_BLOCKED);
    }

    if let Some(tid) = task_id {
        crate::db::set_task_status(&ctx.conn, tid, "dev_done")?;
    }
    Ok(crate::error::EXIT_OK)
}
