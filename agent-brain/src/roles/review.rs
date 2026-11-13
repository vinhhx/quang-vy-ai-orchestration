use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};
use crate::report::ReviewKind;
use crate::roles::RunOpts;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub const REVIEW_ALLOWED_BASE: &[&str] =
    crate::with_git_read_only_tools!("Read", "Grep", "Glob");

pub const REVIEW_DISALLOWED_TOOLS: [&str; 4] = ["WebFetch", "WebSearch", "NotebookEdit", "Task"];

/// WR-05: a role writes its report skeleton before claude runs. While this
/// guard is armed, dropping it puts the report back: `git checkout HEAD --`
/// when the report is tracked at HEAD, removal when it is not. Only the
/// role's own report is touched, and only on a failed run (the role disarms
/// the guard on every path that keeps the report).
pub(crate) struct ReportRestore {
    root: PathBuf,
    file: String,
    tracked: bool,
    armed: bool,
}

impl ReportRestore {
    pub(crate) fn arm(root: &Path, file: &str, tracked: bool) -> ReportRestore {
        ReportRestore {
            root: root.to_path_buf(),
            file: file.to_string(),
            tracked,
            armed: true,
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReportRestore {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        let file = self.file.as_str();
        let cleaned = if self.tracked {
            match crate::gitops::git(&self.root, &["checkout", "HEAD", "--", file]) {
                Ok(_) => true,
                Err(e) => {
                    crate::term::emit(&format!("Không khôi phục được {file}: {}", e.message));
                    false
                }
            }
        } else {
            match std::fs::remove_file(self.root.join(file)) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => {
                    crate::term::emit(&format!("Không xoá được {file}: {e}"));
                    false
                }
            }
        };
        if cleaned {
            crate::term::emit(&format!("Đã dọn {file} của lượt chạy không thành công."));
        }
    }
}

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
    // D2.2: the task's worktree in worktree mode (`workspace::role_root`), else the repo.
    let proj_path = crate::workspace::role_root(&ctx.conn, &proj, &proj_path)?;

    let protocol = crate::protocol_paths::current(&proj, &proj_path);
    let report_file = protocol.review(kind).to_string();
    let plan_path = proj_path.join(&protocol.plan);
    if !plan_path.is_file() {
        return Err(BrainError::error(
            crate::protocol_paths::plan_not_found_message(&protocol.plan, &proj_path),
        ));
    }

    let text_plan = std::fs::read_to_string(&plan_path)?;
    let meta = crate::planfile::read_meta(&text_plan);
    let task = match meta
        .as_ref()
        .and_then(|m| m.get("task"))
        .and_then(|v| v.as_str())
    {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return Err(BrainError::error(format!(
                "File {} không có metadata hợp lệ",
                protocol.plan
            )))
        }
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
    dirty.remove(&report_file);
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

    let plan_commit = crate::roles::plan_commit_for(&ctx.conn, &proj.id, &proj_path, &task)?;
    let plan_commit = match plan_commit {
        Some(c) => c,
        None => {
            return Err(BrainError::error(format!(
                "Không tìm thấy commit '{task}: plan' trên nhánh hiện tại"
            )));
        }
    };

    let code_anchor = crate::roles::code_anchor_for(&ctx.conn, &proj.id, &proj_path, &task)?
        .unwrap_or_else(|| plan_commit.clone());

    if kind == ReviewKind::Code {
        let code = crate::gitops::diff_code_names(&proj_path, &code_anchor, "HEAD")?;
        if code.is_empty() {
            return Err(BrainError::error("chưa có thay đổi code sau plan"));
        }
    }

    let diff_cmd = protocol.code_diff_command(&code_anchor);

    let file = report_file.as_str();
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

    let mut user_input = match kind {
        ReviewKind::Plan => format!(
            "Task: {task}\nĐầu vào: {plan} (commit {plan_commit})\n{prev_line}Ghi {file} theo khung có sẵn: điền front matter verdict / blocking / advisory / files ở đầu file; verdict là revise khi và chỉ khi có ít nhất một mục chặn.",
            plan = protocol.plan
        ),
        ReviewKind::Code => format!(
            "Task: {task}\nĐầu vào: {plan} và kết quả lệnh: {diff_cmd}\n{prev_line}Ghi {file} theo khung có sẵn: điền front matter verdict / blocking / advisory / files ở đầu file; verdict là revise khi và chỉ khi có ít nhất một mục chặn.",
            plan = protocol.plan
        ),
    };
    let skill_block = crate::skills::prompt_block(&crate::skills::discover(&proj_path));
    if !skill_block.is_empty() {
        user_input.push_str("\n\n");
        user_input.push_str(&skill_block);
    }

    let role_enum = match kind {
        ReviewKind::Plan => crate::roles::Role::PlanReview,
        ReviewKind::Code => crate::roles::Role::CodeReview,
    };

    let (sources, _) = crate::projects::read_source_paths(&ctx.map, &proj);
    let source_paths: Vec<std::path::PathBuf> = sources.into_iter().map(|(_, p)| p).collect();

    if opts.dry_run {
        if kind == ReviewKind::Code {
            crate::term::emit_out(&format!("Diff command: {diff_cmd}"));
        }
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
        let workflow = existing_task_id.and_then(|tid| {
            crate::db::task_auto(&ctx.conn, tid)
                .ok()
                .flatten()
                .and_then(|a| a.workflow_json)
                .and_then(|j| crate::workflows::Workflow::from_snapshot(&j))
                .map(|wf| wf.name)
        });
        let team = existing_task_id.and_then(|tid| {
            crate::db::task_team_json(&ctx.conn, tid)
                .ok()
                .flatten()
                .and_then(|j| crate::teams::TaskTeam::parse(Some(&j)))
                .map(|t| t.name)
        });
        let layers = crate::prompts::PromptLayers {
            stacks: proj.stack.clone(),
            project: Some(proj.id.clone()),
            workflow,
            team,
        };
        let spec_dry = crate::roles::RoleSpec {
            role: role_enum,
            user_input: user_input.clone(),
            allowed_tools: allowed,
            disallowed_tools: disallowed,
            extra_args: vec![
                "--settings".to_string(),
                "<run_dir/settings.json>".to_string(),
            ],
            source_paths: source_paths.clone(),
        };
        let (argv, warnings) = crate::roles::build_command_layered(
            &spec_dry,
            &ctx.cfg,
            crate::roles::prompt_for(role_enum),
            &layers,
        )?;
        for w in warnings {
            crate::term::emit(&w);
        }
        crate::term::emit_out(&crate::prompts::dry_run_summary(&layers, kind.role()));
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&argv, &format!("assets/prompts/{}.md", kind.role()))
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_row =
        crate::tasks::current_task_in(&ctx.conn, proj_row_id, &proj_path, &proj.protocol_dir)?;
    let task_id = task_row.map(|t| t.id);

    let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task, kind.role())?;
    let exe = std::env::current_exe()?;
    let settings = crate::guard::write_hook_settings(
        &run_dir,
        &proj_path,
        None,
        &[proj_path.join(&report_file)],
        &exe,
        &source_paths,
    )?;

    let report_path = proj_path.join(&report_file);
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
    // WR-05: every failed exit from here on restores the report.
    let mut restore = ReportRestore::arm(&proj_path, file, has_prev);
    protocol.ensure_dir(&proj_path)?;
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
        source_paths,
    };

    let workflow = task_id.and_then(|tid| {
        crate::db::task_auto(&ctx.conn, tid)
            .ok()
            .flatten()
            .and_then(|a| a.workflow_json)
            .and_then(|j| crate::workflows::Workflow::from_snapshot(&j))
            .map(|wf| wf.name)
    });
    let team = task_id.and_then(|tid| {
        crate::db::task_team_json(&ctx.conn, tid)
            .ok()
            .flatten()
            .and_then(|j| crate::teams::TaskTeam::parse(Some(&j)))
            .map(|t| t.name)
    });
    let layers = crate::prompts::PromptLayers {
        stacks: proj.stack.clone(),
        project: Some(proj.id.clone()),
        workflow,
        team,
    };
    let (argv, warnings) = crate::roles::build_command_layered(
        &spec,
        &ctx.cfg,
        crate::roles::prompt_for(role_enum),
        &layers,
    )?;
    for w in warnings {
        crate::term::emit(&w);
    }
    let model = ctx.cfg.model_for(kind.role())?;
    let task_wf = crate::roles::task_workflow(&ctx.conn, task_id);
    let timeout = crate::roles::task_timeout(ctx, &proj.id, task_wf.as_ref(), kind.role());

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
    extra.remove(&report_file);
    if !extra.is_empty() {
        crate::term::emit_out(&format!(
            "{title} đã sửa file ngoài {} — vi phạm giao thức:",
            report_file
        ));
        let mut sorted_extra: Vec<String> = extra.into_iter().collect();
        sorted_extra.sort();
        for p in sorted_extra {
            crate::term::emit_out(&format!("  {p}"));
        }
        restore.disarm();
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !report_path.is_file() {
        return Err(BrainError::error(format!(
            "{title} không tạo file {}",
            report_file
        )));
    }

    let report_text = std::fs::read_to_string(&report_path)?;
    let report = match crate::report::parse_review(&report_text, kind) {
        Ok(r) => r,
        Err(reason) => {
            crate::term::emit_out(&format!(
                "Báo cáo {} không đúng định dạng: {reason}",
                report_file
            ));
            restore.disarm();
            crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
            return Ok(crate::error::EXIT_PROTOCOL);
        }
    };

    let report_changed = crate::gitops::changed_paths(&proj_path)?.contains(&report_file);
    if report_changed {
        crate::gitops::commit_paths(
            &proj_path,
            &[report_file.as_str()],
            &format!("{task}: {}", kind.role()),
        )?;
    } else {
        crate::term::emit_out(&format!(
            "Báo cáo {} giống bản đã commit — không tạo commit mới.",
            report_file
        ));
    }
    restore.disarm();

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
            report_file
        ));
    }

    Ok(crate::error::EXIT_OK)
}
