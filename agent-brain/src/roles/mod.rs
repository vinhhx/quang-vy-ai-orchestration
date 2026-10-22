pub mod dev;
pub mod lead;
pub mod qa;
pub mod review;

use crate::app::Ctx;
use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use crate::projects::Project;
use crate::runner::RunOutcome;
use crate::stream::Summary;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Role {
    Lead,
    Dev,
    Qa,
    PlanReview,
    CodeReview,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Lead => "lead",
            Role::Dev => "dev",
            Role::Qa => "qa",
            Role::PlanReview => "plan-review",
            Role::CodeReview => "code-review",
        }
    }
}

pub const LEAD_PROMPT: &str = include_str!("../../assets/prompts/lead.md");
pub const DEV_PROMPT: &str = include_str!("../../assets/prompts/dev.md");
pub const DEV_FIX_PROMPT: &str = include_str!("../../assets/prompts/dev-fix.md");
pub const QA_PROMPT: &str = include_str!("../../assets/prompts/qa.md");
pub const PLAN_REVIEW_PROMPT: &str = include_str!("../../assets/prompts/plan-review.md");
pub const CODE_REVIEW_PROMPT: &str = include_str!("../../assets/prompts/code-review.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FixInput {
    Qa,
    CodeReview,
}

impl FixInput {
    pub fn report_file(&self) -> &'static str {
        match self {
            FixInput::Qa => crate::report::QA_REPORT_FILE,
            FixInput::CodeReview => crate::report::CODE_REVIEW_FILE,
        }
    }

    pub fn metrics_value(&self) -> &'static str {
        match self {
            FixInput::Qa => "qa",
            FixInput::CodeReview => "code_review",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FixInput::Qa => "QA",
            FixInput::CodeReview => "review",
        }
    }
}

pub fn prompt_for(role: Role) -> &'static str {
    match role {
        Role::Lead => LEAD_PROMPT,
        Role::Dev => DEV_PROMPT,
        Role::Qa => QA_PROMPT,
        Role::PlanReview => PLAN_REVIEW_PROMPT,
        Role::CodeReview => CODE_REVIEW_PROMPT,
    }
}

pub fn check_forbidden_args(args: &[String]) -> BrainResult<()> {
    for arg in args {
        if arg.starts_with("--dangerously-skip-permissions")
            || arg.starts_with("--allow-dangerously-skip-permissions")
            || arg.starts_with("--permission-mode")
            || arg.contains("bypassPermissions")
        {
            return Err(BrainError::error(format!(
                "Cờ quyền không được phép: {arg}"
            )));
        }
    }
    Ok(())
}

pub struct RoleSpec {
    pub role: Role,
    pub user_input: String,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub extra_args: Vec<String>,
}

pub fn abs_rules(root: &Path, rel_files: &[&str], tools: &[&str]) -> Vec<String> {
    let mut rules = Vec::new();
    for rel in rel_files {
        let clean = rel.strip_prefix("./").unwrap_or(rel);
        let clean = clean.trim_start_matches('/');
        let abs_path = root.join(clean);
        let abs_path_str = abs_path.to_string_lossy();
        for tool in tools {
            rules.push(format!("{tool}(/{abs_path_str})"));
        }
    }
    rules
}

pub fn build_command(spec: &RoleSpec, cfg: &Config) -> BrainResult<Vec<String>> {
    build_command_with_prompt(spec, cfg, prompt_for(spec.role))
}

pub fn build_command_with_prompt(
    spec: &RoleSpec,
    cfg: &Config,
    prompt_text: &str,
) -> BrainResult<Vec<String>> {
    check_forbidden_args(&cfg.common_args)?;
    check_forbidden_args(&spec.extra_args)?;

    let model = cfg.model_for(spec.role.as_str())?;

    let mut cmd = vec![
        cfg.claude_bin.clone(),
        "-p".to_string(),
        spec.user_input.clone(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--model".to_string(),
        model,
        "--append-system-prompt".to_string(),
        prompt_text.to_string(),
    ];

    cmd.extend(cfg.common_args.iter().cloned());

    if let Some(Some(max_turns)) = cfg.max_turns.get(spec.role.as_str()) {
        cmd.push("--max-turns".to_string());
        cmd.push(max_turns.to_string());
    }

    cmd.extend(spec.extra_args.iter().cloned());

    if !spec.allowed_tools.is_empty() {
        cmd.push("--allowedTools".to_string());
        cmd.extend(spec.allowed_tools.iter().cloned());
    }

    if !spec.disallowed_tools.is_empty() {
        cmd.push("--disallowedTools".to_string());
        cmd.extend(spec.disallowed_tools.iter().cloned());
    }

    Ok(cmd)
}

pub fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if !arg.is_empty()
                && arg.chars().all(|c| {
                    c.is_ascii_alphanumeric()
                        || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
                })
            {
                arg.clone()
            } else {
                crate::guard::sh_quote(arg)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn display_argv(argv: &[String], prompt_label: &str) -> String {
    let label = if prompt_label.starts_with('<') && prompt_label.ends_with('>') {
        prompt_label.to_string()
    } else {
        format!("<{prompt_label}>")
    };
    let mut display_cmd = argv.to_vec();
    for i in 0..display_cmd.len() {
        if display_cmd[i] == "--append-system-prompt" && i + 1 < display_cmd.len() {
            display_cmd[i + 1] = label.clone();
            break;
        }
    }
    shell_join(&display_cmd)
}

pub fn new_run_dir(cfg: &Config, project_id: &str, task: &str, role: &str) -> BrainResult<PathBuf> {
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%6fZ").to_string();
    let base = crate::config::runs_dir(cfg);
    let target = base
        .join(project_id)
        .join(task)
        .join(format!("{ts}-{role}"));

    let mut to_create = Vec::new();
    let mut cur = target.clone();
    while !cur.exists() && cur.parent().is_some() {
        to_create.push(cur.clone());
        cur = cur.parent().unwrap().to_path_buf();
    }
    to_create.reverse();
    for d in to_create {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        let _ = builder.create(&d);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
        }
    }
    if !target.exists() {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&target)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700));
    }

    Ok(target)
}

/// Con của driver autopilot (mang AGENT_BRAIN_AUTOPILOT_TASK) chỉ được chạy trên nhánh
/// của đúng task đó (CR-01, D-03, D-15). Không có biến → không kiểm tra, không đọc DB.
/// Chỉ đọc DB (`find_task_by_branch`), không gọi `tasks::current_task` (hàm đó có thể thêm hàng task).
pub fn autopilot_task_guard(
    conn: &rusqlite::Connection,
    project_slug: &str,
    root: &Path,
) -> BrainResult<()> {
    let raw = match std::env::var(crate::autopilot::AUTOPILOT_TASK_ENV) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let value = raw.trim();
    if value.is_empty() {
        return Ok(());
    }
    let want: i64 = value.parse().map_err(|_| {
        BrainError::error(format!(
            "Biến AGENT_BRAIN_AUTOPILOT_TASK không hợp lệ: {value}"
        ))
    })?;

    let cur = crate::gitops::current_branch(root);
    let found = match &cur {
        Some(branch) => match crate::db::project_row_id(conn, project_slug) {
            Ok(pid) => crate::db::find_task_by_branch(conn, pid, branch)?.map(|t| t.id),
            Err(_) => None,
        },
        None => None,
    };
    if found != Some(want) {
        let cur_display = cur.as_deref().unwrap_or("None");
        return Err(BrainError::error(format!(
            "Nhánh hiện tại '{cur_display}' không phải nhánh của task autopilot {want} — dừng, không chạy"
        )));
    }
    Ok(())
}

/// Commit `{slug}: plan` của task đang checkout (IN-09). Có hàng task cho nhánh hiện tại →
/// `plan_commit_on_branch` giới hạn bởi base_commit và `refs/heads/` (giống driver và dashboard),
/// nên commit plan cũ của một task trùng slug đã merge không còn khớp. Không có hàng task
/// (plan cũ chưa vào DB) → quét subject trên HEAD như trước.
pub fn plan_commit_for(
    conn: &rusqlite::Connection,
    project_slug: &str,
    root: &Path,
    task_slug: &str,
) -> BrainResult<Option<String>> {
    if let Some(branch) = crate::gitops::current_branch(root) {
        let row = match crate::db::project_row_id(conn, project_slug) {
            Ok(pid) => crate::db::find_task_by_branch(conn, pid, &branch)?,
            Err(_) => None,
        };
        if let Some(row) = row {
            return crate::gitops::plan_commit_on_branch(
                root,
                &row.branch,
                &row.slug,
                row.base_commit.as_deref(),
            )
            .map_err(|_| {
                BrainError::error(format!("Không đọc được lịch sử git của nhánh {branch}"))
            });
        }
    }
    crate::gitops::find_commit_by_subject(root, &format!("{task_slug}: plan"))
}

pub struct RunPlan {
    pub role: String,
    pub model: String,
    pub argv: Vec<String>,
    pub task_id: Option<i64>,
    pub task_slug: String,
    pub base_commit: Option<String>,
    pub plan_files: Option<Vec<String>>,
    pub verbose: bool,
    pub timeout: Duration,
    pub run_dir: Option<PathBuf>,
}

pub struct RoleRun {
    pub run_id: i64,
    pub outcome: RunOutcome,
    pub summary: Summary,
    pub run_dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct RunStage {
    pub stage: Option<String>,
    pub round: Option<i64>,
    pub metrics: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct RunOpts {
    pub name: Option<String>,
    pub dry_run: bool,
    pub verbose: bool,
    pub round: Option<i64>,
    pub fix: Option<FixInput>,
    pub review: bool,
    pub note: Option<String>,
    pub workflow: Option<String>,
    pub origin: Option<String>,
}

pub fn execute_role(
    ctx: &mut Ctx,
    project: &Project,
    root: &Path,
    plan: RunPlan,
) -> BrainResult<RoleRun> {
    execute_role_staged(ctx, project, root, plan, RunStage::default())
}

pub fn execute_role_staged(
    ctx: &mut Ctx,
    project: &Project,
    root: &Path,
    plan: RunPlan,
    stage: RunStage,
) -> BrainResult<RoleRun> {
    let running = crate::db::running_runs(&ctx.conn)?;
    for r in running {
        if r.profile == project.profile {
            if let (Some(pid), Some(pid_start)) = (r.pid, r.pid_start) {
                let proc_id = crate::procs::ProcId {
                    pid: pid as i32,
                    start: pid_start as u64,
                };
                if crate::procs::alive(proc_id) {
                    if project.profile == "personal" {
                        let tid_str = r
                            .task_id
                            .map(|tid| tid.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        return Err(BrainError::error(format!(
                            "Profile personal đang có run {} chạy (task {tid_str}) — không chạy song song trên cùng thư mục cấu hình (E-06)",
                            r.id
                        )));
                    } else {
                        crate::term::emit(&format!(
                            "Cảnh báo: profile {} đang có run {} chạy song song",
                            project.profile, r.id
                        ));
                    }
                }
            }
        }
    }

    // Driver autopilot còn sống (giữa hai bước không có run nào chạy) chặn run tay:
    // luật cùng dự án áp cho mọi profile (IN-10), luật E-06 chỉ cho profile personal.
    // Con của chính driver đó mang AGENT_BRAIN_AUTOPILOT_TASK nên được bỏ qua.
    let exclude = std::env::var(crate::autopilot::AUTOPILOT_TASK_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok());
    use crate::autopilot::claim::live_driver_reason;
    let live = live_driver_reason(&ctx.conn, &project.id, &project.profile, exclude)?;
    if let Some(reason) = live {
        return Err(BrainError::error(reason));
    }

    let env = crate::profile::build_env(
        &project.profile,
        &ctx.cfg.profiles,
        &crate::profile::current_env(),
    )?;
    let run_dir = match &plan.run_dir {
        Some(rd) => {
            std::fs::create_dir_all(rd)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(rd, std::fs::Permissions::from_mode(0o700));
            }
            rd.clone()
        }
        None => new_run_dir(&ctx.cfg, &project.id, &plan.task_slug, &plan.role)?,
    };

    let p_desc = crate::profile::describe_profile(&project.profile, &ctx.cfg.profiles);
    crate::term::emit(&format!(
        "[agent-brain] {} · {} · profile {} ({}) · model {}",
        plan.role, project.id, project.profile, p_desc, plan.model
    ));

    let stderr_path = run_dir.join("stderr.txt");
    let stream_path = run_dir.join("stream.jsonl");

    let req = crate::runner::RunRequest {
        argv: plan.argv.clone(),
        cwd: root.to_path_buf(),
        env,
        stderr_path,
        stream_path,
        timeout: plan.timeout,
        kill_grace: Duration::from_secs(ctx.cfg.kill_grace_secs),
    };

    struct Observer<'a> {
        ctx: &'a Ctx,
        plan: &'a RunPlan,
        stage: &'a RunStage,
        project_id: &'a str,
        profile: &'a str,
        run_dir: &'a Path,
        summary: Summary,
        run_id: Option<i64>,
        seq: i64,
    }

    impl<'a> crate::runner::RunObserver for Observer<'a> {
        fn on_spawn(&mut self, pid: i32, pgid: i32, pid_start: u64) -> BrainResult<()> {
            let proj_row_id = crate::db::project_row_id(&self.ctx.conn, self.project_id)?;
            let new_run = crate::db::NewRun {
                task_id: self.plan.task_id,
                project_id: proj_row_id,
                role: self.plan.role.clone(),
                profile: self.profile.to_string(),
                model: Some(self.plan.model.clone()),
                pid: pid as i64,
                pgid: pgid as i64,
                pid_start: pid_start as i64,
                base_commit: self.plan.base_commit.clone(),
                run_dir: Some(self.run_dir.to_string_lossy().to_string()),
            };
            let run_id = crate::db::insert_run(&self.ctx.conn, &new_run)?;
            self.run_id = Some(run_id);
            if self.stage.stage.is_some() || self.stage.round.is_some() {
                crate::db::set_run_stage_round(
                    &self.ctx.conn,
                    run_id,
                    self.stage.stage.as_deref(),
                    self.stage.round,
                )?;
            }
            if let Some(ref m) = self.stage.metrics {
                crate::db::merge_run_metrics(&self.ctx.conn, run_id, m)?;
            }
            if let Some(tid) = self.plan.task_id {
                crate::term::emit(&format!("task {tid}, run {run_id}"));
            } else {
                crate::term::emit(&format!("run {run_id}"));
            }
            Ok(())
        }

        fn on_line(&mut self, line: &[u8]) {
            if let Some(ev) = self.summary.feed(line) {
                self.seq += 1;
                if let Some(run_id) = self.run_id {
                    let _ = crate::db::insert_event(
                        &self.ctx.conn,
                        run_id,
                        self.seq,
                        crate::db::now_ms(),
                        &ev.kind,
                        &ev.raw,
                    );
                }
                let line_text = crate::term::event_line(&ev, self.plan.verbose);
                crate::term::emit(&line_text);
            }
        }

        fn on_notice(&mut self, message: &str) {
            crate::term::emit(message);
        }

        fn kill_requested(&mut self) -> bool {
            if let Some(run_id) = self.run_id {
                let req: Option<i64> = self
                    .ctx
                    .conn
                    .query_row(
                        "SELECT kill_requested_at FROM runs WHERE id = ?1",
                        rusqlite::params![run_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(None);
                req.is_some()
            } else {
                false
            }
        }
    }

    let mut obs = Observer {
        ctx,
        plan: &plan,
        stage: &stage,
        project_id: &project.id,
        profile: &project.profile,
        run_dir: &run_dir,
        summary: Summary::new(plan.plan_files.clone(), Some(root.to_path_buf())),
        run_id: None,
        seq: 0,
    };

    let outcome = crate::runner::supervise(&req, &mut obs)?;
    obs.summary.finish();

    if let Some(run_id) = obs.run_id {
        let finish = crate::db::RunFinish {
            status: outcome.status.as_str().to_string(),
            exit_code: outcome.exit_code,
            session_id: if obs.summary.session_id.is_empty() {
                None
            } else {
                Some(obs.summary.session_id.clone())
            },
            usage: obs.summary.usage,
            usage_source: obs.summary.usage_source.as_str().to_string(),
            cost_usd: obs.summary.cost_usd,
            num_turns: obs.summary.num_turns,
            assistant_turns: Some(obs.summary.assistant_turns),
            turns_before_first_edit: obs.summary.turns_before_first_edit,
            reads_outside_plan: obs
                .summary
                .reads_outside_plan
                .as_ref()
                .map(|v| v.len() as i64),
            permission_denials: obs.summary.permission_denials,
            error: None,
        };
        crate::db::finish_run(&ctx.conn, run_id, &finish)?;
        crate::term::emit(&crate::term::summary_line(
            run_id,
            plan.task_id,
            outcome.status.as_str(),
            &obs.summary,
            outcome.elapsed,
        ));
    }

    let run_id = obs.run_id.unwrap_or(0);
    Ok(RoleRun {
        run_id,
        outcome,
        summary: obs.summary,
        run_dir,
    })
}

pub fn cmd_run(
    ctx: &mut Ctx,
    role: Role,
    project: &str,
    request: Option<&str>,
    opts: &RunOpts,
) -> BrainResult<i32> {
    if let Some(r) = opts.round {
        if r < 1 {
            return Err(BrainError::error("--round phải là số nguyên từ 1 trở lên"));
        }
    }

    if opts.fix.is_some() && role != Role::Dev {
        return Err(BrainError::error("--fix chỉ dùng với vai trò dev"));
    }

    if (opts.review || opts.note.is_some() || opts.workflow.is_some()) && role != Role::Lead {
        return Err(BrainError::error(
            "--review, --note và --workflow chỉ dùng với vai trò lead",
        ));
    }

    if opts.origin.is_some() && role != Role::Lead {
        return Err(BrainError::error("--origin chỉ dùng với vai trò lead"));
    }

    match role {
        Role::Lead => {
            let req = match request {
                Some(r) if !r.trim().is_empty() => r.trim(),
                _ => {
                    return Err(BrainError::error(
                        "run lead cần <yêu cầu> — ví dụ: agent-brain run lead \"<dự án>\" \"<yêu cầu>\"",
                    ));
                }
            };
            lead::cmd_lead(ctx, project, req, opts)
        }
        Role::Dev => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(
                        "Bỏ qua <yêu cầu> cho vai trò dev: vai trò này chỉ đọc .plan.md / git diff",
                    );
                }
            }
            dev::cmd_dev(ctx, project, opts)
        }
        Role::Qa => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(
                        "Bỏ qua <yêu cầu> cho vai trò qa: vai trò này chỉ đọc .plan.md / git diff",
                    );
                }
            }
            qa::cmd_qa(ctx, project, opts)
        }
        Role::PlanReview => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(&format!(
                        "Bỏ qua <yêu cầu> cho vai trò {}: vai trò này chỉ đọc .plan.md / git diff",
                        role.as_str()
                    ));
                }
            }
            review::cmd_review(ctx, project, crate::report::ReviewKind::Plan, opts)
        }
        Role::CodeReview => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(&format!(
                        "Bỏ qua <yêu cầu> cho vai trò {}: vai trò này chỉ đọc .plan.md / git diff",
                        role.as_str()
                    ));
                }
            }
            review::cmd_review(ctx, project, crate::report::ReviewKind::Code, opts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_join() {
        assert_eq!(shell_join(&[]), "");
        assert_eq!(
            shell_join(&["hello".to_string(), "world".to_string()]),
            "hello world"
        );
        assert_eq!(shell_join(&["a b".to_string(), "c".to_string()]), "'a b' c");
        assert_eq!(
            shell_join(&["Write(./.plan.md)".to_string()]),
            "'Write(./.plan.md)'"
        );
    }

    #[test]
    fn test_display_argv() {
        let argv = vec![
            "claude".to_string(),
            "--append-system-prompt".to_string(),
            "very long prompt".to_string(),
            "--model".to_string(),
            "opus".to_string(),
        ];
        let disp = display_argv(&argv, "assets/prompts/lead.md");
        assert!(disp.contains("<assets/prompts/lead.md>"));
        assert!(!disp.contains("very long prompt"));
    }
}
