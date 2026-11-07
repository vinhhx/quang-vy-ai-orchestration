use crate::app::Ctx;
use crate::error::{BrainError, BrainResult};

pub const QA_ALLOWED_TOOLS: &[&str] = crate::with_git_read_only_tools!(
    "Read",
    "Grep",
    "Glob",
    "Write(./.qa-report.md)",
    "Edit(./.qa-report.md)",
);

pub const QA_DISALLOWED_TOOLS: [&str; 4] = ["WebFetch", "WebSearch", "NotebookEdit", "Task"];

/// Heading of the section agent-brain appends to `.qa-report.md` after checking the products.
pub const PRODUCT_CHECK_HEADING: &str = "## Kiểm tra sản phẩm (agent-brain)";

/// One product file checked on disk: its path resolved through symlinks and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductFound {
    pub path: std::path::PathBuf,
    pub size: u64,
}

/// Checks one `## Sản phẩm` file deterministically: it must exist, resolve (through symlinks)
/// inside the canonical project root, be a regular file and hold at least one byte. `Err` is
/// the Vietnamese reason.
pub fn check_product(root: &std::path::Path, rel: &str) -> Result<ProductFound, &'static str> {
    let path = root.join(rel);
    if std::fs::symlink_metadata(&path).is_err() {
        return Err("không tồn tại");
    }
    let Ok(resolved) = path.canonicalize() else {
        // a symlink whose target is missing
        return Err("không tồn tại");
    };
    let Ok(root_real) = root.canonicalize() else {
        return Err("ra ngoài gốc dự án");
    };
    if resolved == root_real || !resolved.starts_with(&root_real) {
        return Err("ra ngoài gốc dự án");
    }
    let meta = match std::fs::metadata(&resolved) {
        Ok(m) => m,
        Err(_) => return Err("không tồn tại"),
    };
    if !meta.is_file() {
        return Err("không phải file thường");
    }
    if meta.len() == 0 {
        return Err("rỗng (0 byte)");
    }
    Ok(ProductFound {
        path: resolved,
        size: meta.len(),
    })
}

/// The `## Sản phẩm` files of a task: those of the plan at each commit in `commits` (the
/// approved plan first), then those of `head_plan_text`, without duplicates. A plan edited after
/// the approval can add products but never drop one.
pub fn task_product_files(
    root: &std::path::Path,
    commits: &[&str],
    head_plan_text: Option<&str>,
) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    for c in commits {
        if !crate::gitops::is_hex_sha(c) {
            continue;
        }
        if let Ok(t) = crate::gitops::git(root, &["show", &format!("{c}:.plan.md")]) {
            texts.push(t);
        }
    }
    if let Some(t) = head_plan_text {
        texts.push(t.to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for t in &texts {
        for f in crate::planfile::product_files(&crate::planfile::parse_plan(t)) {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

/// `(file, outcome)` for each product file, in order.
pub fn check_products(
    root: &std::path::Path,
    files: &[String],
) -> Vec<(String, Result<ProductFound, &'static str>)> {
    files
        .iter()
        .map(|f| (f.clone(), check_product(root, f)))
        .collect()
}

/// `text` with every verdict line under `## Kết luận` replaced by `verdict`; nothing else
/// changes. When the section has no verdict line, `verdict` is put right under the heading.
pub fn set_qa_verdict(text: &str, verdict: &str) -> String {
    let norm_heading = |line: &str| -> Option<String> {
        if line.starts_with("## ") || line.starts_with("# ") {
            Some(
                unicode_normalization::UnicodeNormalization::nfc(
                    line.trim_start_matches('#').trim(),
                )
                .collect(),
            )
        } else {
            None
        }
    };
    let is_verdict_line = |line: &str| -> bool {
        let one = format!("## Kết luận\n{line}\n");
        crate::planfile::parse_qa_verdict(&one).is_some()
    };
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut heading_at: Option<usize> = None;
    let mut replaced = false;
    for line in text.split('\n') {
        if let Some(h) = norm_heading(line) {
            if in_section {
                in_section = false;
            } else if heading_at.is_none() && h.starts_with("Kết luận") {
                in_section = true;
                out.push(line.to_string());
                heading_at = Some(out.len() - 1);
                continue;
            }
        } else if in_section && is_verdict_line(line) {
            out.push(verdict.to_string());
            replaced = true;
            continue;
        }
        out.push(line.to_string());
    }
    match (replaced, heading_at) {
        (false, Some(i)) => out.insert(i + 1, verdict.to_string()),
        (false, None) => {
            if out.last().is_some_and(|l| l.is_empty()) {
                out.pop();
            }
            out.push("## Kết luận".to_string());
            out.push(verdict.to_string());
            out.push(String::new());
        }
        _ => {}
    }
    out.join("\n")
}

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

    let plan_commit = crate::roles::plan_commit_for(&ctx.conn, &proj.id, &proj_path, &task)?;
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

    let (sources, _) = crate::projects::read_source_paths(&ctx.map, &proj);
    let source_paths: Vec<std::path::PathBuf> = sources.into_iter().map(|(_, p)| p).collect();

    if dry_run {
        crate::term::emit_out(&format!("Diff command: {diff_cmd}"));
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
        let mut user_input = format!(
            "Task: {task}\nĐầu vào: .plan.md và kết quả lệnh: {diff_cmd}\nGhi .qa-report.md theo khung có sẵn; mục Kết luận ghi đúng một dòng: Đạt hoặc Cần sửa.\nỞ đầu .qa-report.md (trước dòng \"# QA:\") thêm front matter: dòng ---, specs: <số kiểm tra đã chạy>, failing: <số kiểm tra lỗi>, not_automatable: <số mục không kiểm tự động được>, dòng ---. Bỏ dòng nào bạn không biết."
        );
        let skill_block = crate::skills::prompt_block(&crate::skills::discover(&proj_path));
        if !skill_block.is_empty() {
            user_input.push_str("\n\n");
            user_input.push_str(&skill_block);
        }
        let spec_dry = crate::roles::RoleSpec {
            role: crate::roles::Role::Qa,
            user_input,
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
            crate::roles::prompt_for(crate::roles::Role::Qa),
            &layers,
        )?;
        for w in warnings {
            crate::term::emit(&w);
        }
        crate::term::emit_out(&crate::prompts::dry_run_summary(&layers, "qa"));
        crate::term::emit_out(&format!(
            "Command: {}",
            crate::roles::display_argv(&argv, "assets/prompts/qa.md")
        ));
        return Ok(crate::error::EXIT_OK);
    }

    let proj_row_id = crate::db::project_row_id(&ctx.conn, &proj.id)?;
    let task_row = crate::tasks::current_task(&ctx.conn, proj_row_id, &proj_path)?;
    let task_id = task_row.map(|t| t.id);

    // W-1: QA's Bash git calls go through the guard's git policy; its file
    // writes stay with the allow rules and the post-run backstop.
    let run_dir = crate::roles::new_run_dir(&ctx.cfg, &proj.id, &task, "qa")?;
    let exe = std::env::current_exe()?;
    let settings = crate::guard::write_bash_hook_settings(&run_dir, &proj_path, &exe)?;

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
    // WR-05: every failed exit from here on restores .qa-report.md.
    let tracked = crate::gitops::git(&proj_path, &["cat-file", "-e", "HEAD:.qa-report.md"]).is_ok();
    let mut restore =
        crate::roles::review::ReportRestore::arm(&proj_path, ".qa-report.md", tracked);
    std::fs::write(&qa_path, crate::planfile::render_qa_skeleton(req, &qa_meta))?;

    let mut user_input = format!(
        "Task: {task}\nĐầu vào: .plan.md và kết quả lệnh: {diff_cmd}\nGhi .qa-report.md theo khung có sẵn; mục Kết luận ghi đúng một dòng: Đạt hoặc Cần sửa.\nỞ đầu .qa-report.md (trước dòng \"# QA:\") thêm front matter: dòng ---, specs: <số kiểm tra đã chạy>, failing: <số kiểm tra lỗi>, not_automatable: <số mục không kiểm tự động được>, dòng ---. Bỏ dòng nào bạn không biết."
    );
    let skill_block = crate::skills::prompt_block(&crate::skills::discover(&proj_path));
    if !skill_block.is_empty() {
        user_input.push_str("\n\n");
        user_input.push_str(&skill_block);
    }

    let spec = crate::roles::RoleSpec {
        role: crate::roles::Role::Qa,
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
        crate::roles::prompt_for(crate::roles::Role::Qa),
        &layers,
    )?;
    for w in warnings {
        crate::term::emit(&w);
    }
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
        run_dir: Some(run_dir),
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
        restore.disarm();
        crate::db::set_run_outcome(&ctx.conn, role_run.run_id, "protocol_violation")?;
        return Ok(crate::error::EXIT_PROTOCOL);
    }

    if !qa_path.is_file() {
        return Err(BrainError::error("QA không tạo file .qa-report.md"));
    }

    let mut qa_text = std::fs::read_to_string(&qa_path)?;
    let verdict = crate::planfile::parse_qa_verdict(&qa_text);
    let mut verdict = match verdict {
        Some(v) => v,
        None => {
            // A report without a verdict is kept as evidence, like the
            // reviewers' malformed report (protocol_violation).
            restore.disarm();
            return Err(BrainError::error("QA chưa ghi Kết luận (Đạt / Cần sửa)"));
        }
    };

    // `## Sản phẩm`: checked on disk by agent-brain, whatever the QA wrote.
    let approved = match task_id {
        Some(tid) => crate::db::task_auto(&ctx.conn, tid)?.and_then(|a| a.approved_commit),
        None => None,
    };
    let mut plan_commits: Vec<&str> = Vec::new();
    if let Some(a) = approved.as_deref() {
        plan_commits.push(a);
    }
    plan_commits.push(&plan_commit);
    let products = task_product_files(&proj_path, &plan_commits, Some(&text_plan));
    if !products.is_empty() {
        let checked = check_products(&proj_path, &products);
        let missing = checked.iter().any(|(_, r)| r.is_err());
        let mut section = format!("\n{PRODUCT_CHECK_HEADING}\n");
        for (f, r) in &checked {
            match r {
                Ok(found) => section.push_str(&format!("- {f}: có, {} byte\n", found.size)),
                Err(reason) => section.push_str(&format!("- {f}: {reason}\n")),
            }
        }
        if missing {
            qa_text = set_qa_verdict(&qa_text, "Cần sửa");
            verdict = "Cần sửa";
            crate::term::emit_out("Thiếu sản phẩm — QA chuyển thành Cần sửa");
        }
        if !qa_text.ends_with('\n') {
            qa_text.push('\n');
        }
        qa_text.push_str(&section);
        std::fs::write(&qa_path, &qa_text)?;
    }

    crate::gitops::commit_paths(&proj_path, &[".qa-report.md"], &format!("{task}: qa"))?;
    restore.disarm();
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
            if crate::db::end_stopped_autopilot(&ctx.conn, tid)? {
                crate::term::emit(&format!(
                    "Autopilot của task {tid} đang dừng — task đã qua QA nên tắt autopilot"
                ));
            }
        }
    }

    Ok(crate::error::EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_qa_verdict_replaces_only_the_verdict_line() {
        let text = "---\nspecs: 2\n---\n# QA: t\n## Rủi ro\nĐạt\n## Kết luận\n- **Đạt**\n\n## Ghi chú\nĐạt\n";
        let out = set_qa_verdict(text, "Cần sửa");
        assert_eq!(
            out,
            "---\nspecs: 2\n---\n# QA: t\n## Rủi ro\nĐạt\n## Kết luận\nCần sửa\n\n## Ghi chú\nĐạt\n"
        );
        assert_eq!(crate::planfile::parse_qa_verdict(&out), Some("Cần sửa"));
        // `Kết luận: Đạt` form, at the end of the file
        let out2 = set_qa_verdict("# QA: t\n## Kết luận\nKết luận: Đạt", "Cần sửa");
        assert_eq!(out2, "# QA: t\n## Kết luận\nCần sửa");
        // no verdict line: put under the heading
        let out3 = set_qa_verdict("# QA: t\n## Kết luận\nghi chú\n", "Cần sửa");
        assert_eq!(out3, "# QA: t\n## Kết luận\nCần sửa\nghi chú\n");
        // no section at all
        let out4 = set_qa_verdict("# QA: t\n", "Cần sửa");
        assert_eq!(crate::planfile::parse_qa_verdict(&out4), Some("Cần sửa"));
    }

    #[test]
    fn check_product_reasons() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let other = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.join("out/d")).unwrap();
        std::fs::write(root.join("out/ok.bin"), b"abc").unwrap();
        std::fs::write(root.join("out/empty.bin"), b"").unwrap();
        std::fs::write(other.path().join("x.bin"), b"abc").unwrap();
        std::os::unix::fs::symlink(other.path().join("x.bin"), root.join("out/away.bin")).unwrap();
        std::os::unix::fs::symlink(root.join("out/ok.bin"), root.join("out/inside.bin")).unwrap();
        std::os::unix::fs::symlink(root.join("out/none"), root.join("out/dangling.bin")).unwrap();

        assert_eq!(check_product(root, "out/ok.bin").unwrap().size, 3);
        assert_eq!(check_product(root, "out/inside.bin").unwrap().size, 3);
        assert_eq!(check_product(root, "out/none.bin"), Err("không tồn tại"));
        assert_eq!(
            check_product(root, "out/dangling.bin"),
            Err("không tồn tại")
        );
        assert_eq!(
            check_product(root, "out/away.bin"),
            Err("ra ngoài gốc dự án")
        );
        assert_eq!(check_product(root, "out/d"), Err("không phải file thường"));
        assert_eq!(check_product(root, "out/empty.bin"), Err("rỗng (0 byte)"));
    }
}
