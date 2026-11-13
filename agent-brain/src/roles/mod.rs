pub mod analyst;
pub mod dev;
pub mod lead;
pub mod qa;
pub mod review;

use crate::app::Ctx;
use crate::config::Config;
use crate::error::{BrainError, BrainResult, EXIT_ENV_DOWN, EXIT_PROTOCOL, EXIT_QUOTA};
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
    Analyst,
    AnalysisReview,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Lead => "lead",
            Role::Dev => "dev",
            Role::Qa => "qa",
            Role::PlanReview => "plan-review",
            Role::CodeReview => "code-review",
            Role::Analyst => "analyst",
            Role::AnalysisReview => "analysis-review",
        }
    }

    pub fn config_role(&self) -> Role {
        match self {
            Role::Analyst => Role::Lead,
            Role::AnalysisReview => Role::PlanReview,
            other => *other,
        }
    }
}

pub const LEAD_PROMPT: &str = include_str!("../../assets/prompts/lead.md");
/// Extra Lead rules for requests that carry forwarded (untrusted) text; appended to
/// `LEAD_PROMPT` at run time by `lead_untrusted_prompt`, never a full copy of lead.md.
pub const LEAD_UNTRUSTED_RULES: &str = include_str!("../../assets/prompts/lead-untrusted.md");
pub const DEV_PROMPT: &str = include_str!("../../assets/prompts/dev.md");
pub const DEV_FIX_PROMPT: &str = include_str!("../../assets/prompts/dev-fix.md");
pub const QA_PROMPT: &str = include_str!("../../assets/prompts/qa.md");
pub const PLAN_REVIEW_PROMPT: &str = include_str!("../../assets/prompts/plan-review.md");
pub const CODE_REVIEW_PROMPT: &str = include_str!("../../assets/prompts/code-review.md");
pub const ANALYST_PROMPT: &str = include_str!("../../assets/prompts/analyst.md");
pub const ANALYSIS_REVIEW_PROMPT: &str = include_str!("../../assets/prompts/analysis-review.md");

/// Seconds an agy run gets to exit by itself after its final `result` event; after that the
/// runner stops its process tree (an MCP server agy started can keep it alive) and the run counts
/// as ended by itself.
pub const AGY_AFTER_RESULT_GRACE_SECS: u64 = 15;

/// Same for every other engine (claude, claude-format endpoints): a CLI that already printed its
/// final `result` but does not exit is stopped after this many seconds.
pub const AFTER_RESULT_GRACE_SECS: u64 = 30;

/// Seconds a run of `stream` gets to exit after its final `result` event.
pub fn after_result_grace(stream: crate::engine::StreamFormat) -> Duration {
    Duration::from_secs(if stream == crate::engine::StreamFormat::Agy {
        AGY_AFTER_RESULT_GRACE_SECS
    } else {
        AFTER_RESULT_GRACE_SECS
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FixInput {
    Qa,
    CodeReview,
}

impl FixInput {
    pub fn report_file(&self, protocol: &crate::protocol_paths::ProtocolPaths) -> String {
        match self {
            FixInput::Qa => protocol.qa_report.clone(),
            FixInput::CodeReview => protocol.code_review.clone(),
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
        Role::Analyst => ANALYST_PROMPT,
        Role::AnalysisReview => ANALYSIS_REVIEW_PROMPT,
    }
}

/// `LEAD_PROMPT` (trailing newlines trimmed) + a blank line + the untrusted-data rules.
pub fn lead_untrusted_prompt() -> String {
    format!(
        "{}\n\n{}",
        LEAD_PROMPT.trim_end_matches('\n'),
        LEAD_UNTRUSTED_RULES
    )
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
    pub source_paths: Vec<PathBuf>,
}

/// `base` with `Write(./<file>)` and `Edit(./<file>)` for the role's one protocol file put right
/// after `Glob` (the read tools come first, as in the historical lists).
pub fn with_protocol_write_rules(base: &[&str], file: &str) -> Vec<String> {
    let mut out: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    let at = out.iter().position(|s| s == "Glob").map(|i| i + 1).unwrap_or(0);
    out.insert(at, format!("Edit(./{file})"));
    out.insert(at, format!("Write(./{file})"));
    out
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

pub fn build_command_layered(
    spec: &RoleSpec,
    cfg: &Config,
    base_prompt: &str,
    layers: &crate::prompts::PromptLayers,
) -> BrainResult<(Vec<String>, Vec<String>)> {
    let (composed, warnings) = crate::prompts::compose(base_prompt, spec.role.as_str(), layers);
    let cmd = build_command_with_prompt(spec, cfg, &composed)?;
    Ok((cmd, warnings))
}

pub fn build_command_with_prompt(
    spec: &RoleSpec,
    cfg: &Config,
    prompt_text: &str,
) -> BrainResult<Vec<String>> {
    check_forbidden_args(&cfg.common_args)?;
    check_forbidden_args(&spec.extra_args)?;

    let cfg_role = spec.role.config_role();
    let model = cfg.model_for(cfg_role.as_str())?;

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

    if let Some(Some(max_turns)) = cfg.max_turns.get(cfg_role.as_str()) {
        cmd.push("--max-turns".to_string());
        cmd.push(max_turns.to_string());
    }

    cmd.extend(spec.extra_args.iter().cloned());

    for src in &spec.source_paths {
        cmd.push("--add-dir".to_string());
        cmd.push(src.to_string_lossy().to_string());
    }

    let mut allowed = spec.allowed_tools.clone();
    let mut disallowed = spec.disallowed_tools.clone();
    for src in &spec.source_paths {
        allowed.extend(abs_rules(src, &["**"], &["Read"]));
        disallowed.extend(abs_rules(src, &["**/.env*", ".env*", ".git/**"], &["Read"]));
        disallowed.extend(abs_rules(src, &["**"], &["Edit", "Write"]));
    }

    if !allowed.is_empty() {
        cmd.push("--allowedTools".to_string());
        cmd.extend(allowed);
    }

    if !disallowed.is_empty() {
        cmd.push("--disallowedTools".to_string());
        cmd.extend(disallowed);
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

/// Commit anchor dùng để kiểm tra "có code sau plan" và tính diff code cho Review / QA (3.2).
/// Neo ở commit plan ĐẦU TIÊN của task trên nhánh hiện tại (first_plan_commit_on_branch)
/// để không mất code làm ở các round trước khi Lead lập lại plan.
/// Không tìm được trên nhánh task → fallback về `find_commit_by_subject`.
pub fn code_anchor_for(
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
            return crate::gitops::first_plan_commit_on_branch(
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

/// File in the run dir holding the run's deadline (epoch ms).
pub const DEADLINE_FILE: &str = "deadline";

/// `N phút` for whole minutes, else `N phút S giây` / `S giây`.
fn duration_text(secs: u64) -> String {
    let (m, s) = (secs / 60, secs % 60);
    match (m, s) {
        (0, s) => format!("{s} giây"),
        (m, 0) => format!("{m} phút"),
        (m, s) => format!("{m} phút {s} giây"),
    }
}

/// `HH:MM` local time of an epoch-ms instant.
pub fn local_hhmm(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

/// The deadline notice added to every role prompt (C1). Every number comes from the run's limit
/// and the engine's `tool_timeout_seconds`; without the latter there is no shell line.
pub fn deadline_prompt_block(
    timeout: Duration,
    end_hhmm: &str,
    tool_timeout_secs: Option<u64>,
) -> String {
    let mut block = format!(
        "## Thời hạn\nBạn có {} cho lần chạy này (hết lúc {end_hhmm}). Hãy commit hoặc ghi kết quả trước khi hết giờ.",
        duration_text(timeout.as_secs())
    );
    if let Some(t) = tool_timeout_secs {
        block.push_str(&format!(
            "\nMỗi lệnh shell bị cắt sau {}. Lệnh dài hơn phải chia nhỏ, không chạy nền.",
            duration_text(t)
        ));
    }
    block
}

/// Error text of a run stopped because its environment went down (C2).
fn env_down_message(reason: &str) -> String {
    format!(
        "Môi trường không còn sẵn sàng trong lúc chạy ({} lần kiểm liên tiếp lỗi) — {reason}",
        crate::preflight::FAILS_TO_STOP
    )
}

/// The workflow snapshot stored on a task, if any.
pub fn task_workflow(
    conn: &rusqlite::Connection,
    task_id: Option<i64>,
) -> Option<crate::workflows::Workflow> {
    let tid = task_id?;
    crate::db::task_auto(conn, tid)
        .ok()
        .flatten()
        .and_then(|a| a.workflow_json)
        .and_then(|j| crate::workflows::Workflow::from_snapshot(&j))
}

/// C1: a role's run limit on a task: project `timeouts` > workflow `timeouts` >
/// `config.timeout_seconds` > built-in default.
pub fn task_timeout(
    ctx: &Ctx,
    project_id: &str,
    workflow: Option<&crate::workflows::Workflow>,
    role: &str,
) -> Duration {
    let project = crate::projects::timeouts_of(&ctx.map, project_id);
    let empty = std::collections::BTreeMap::new();
    let wf = workflow.map(|w| &w.timeouts).unwrap_or(&empty);
    ctx.cfg.timeout_for_task(&project, wf, role)
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
    pub warnings: Vec<String>,
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
    pub team: Option<String>,
    pub level: Option<String>,
    /// Dev only: the closing turn after a Dev timeout (workflow `finalize_on_timeout`).
    pub finalize: bool,
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
    mut plan: RunPlan,
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
    // D2.2: in worktree mode tasks do not share a tree, so another task's driver only blocks
    // when the project already runs `max_parallel_tasks` tasks besides this one.
    use crate::autopilot::claim::live_driver_reason;
    let live = if project.workspace.is_worktree() {
        crate::autopilot::claim::worktree_busy_reason(
            &ctx.conn,
            &project.id,
            &project.profile,
            plan.task_id.or(exclude),
            project.workspace.max_parallel_tasks,
            &[],
        )?
    } else {
        live_driver_reason(&ctx.conn, &project.id, &project.profile, exclude)?
    };
    if let Some(reason) = live {
        return Err(BrainError::error(reason));
    }

    // ENG-01 / D-14: the one seam that decides the engine of a run. It runs before any run row
    // is inserted and before anything is spawned.
    let resolved =
        crate::engine::resolve_for_run_with(&ctx.conn, plan.task_id, &plan.role, Some(&ctx.cfg))?;

    // C2: the project's target environment, for the roles in `preflight_roles`, before any run
    // row exists. A project without `preflight` skips this entirely.
    let map = crate::projects::load_map(&crate::config::map_path()).unwrap_or_default();
    let preflight = crate::projects::preflight_of(&map, &project.id);
    let env_watch = if preflight.applies_to(&plan.role) {
        if let Err(reason) = crate::preflight::run_checks(&preflight.checks) {
            return Err(BrainError::new(
                crate::preflight::refusal_message(&reason),
                EXIT_ENV_DOWN,
            ));
        }
        (preflight.interval_seconds > 0).then(|| crate::runner::EnvWatch {
            interval: Duration::from_secs(preflight.interval_seconds),
            checks: preflight.checks.clone(),
        })
    } else {
        None
    };

    let (dept_extra, dept_warns) = crate::departments::prompt_append_for_member(
        &plan.role,
        resolved.member.as_ref().map(|p| &p.member),
    );
    for w in &dept_warns {
        crate::term::emit(w);
    }
    crate::engine::append_to_system_prompt(&mut plan.argv, &dept_extra);

    let (sources_block, source_warnings) =
        crate::projects::read_sources_prompt_block(&map, project);
    for w in &source_warnings {
        crate::term::emit(w);
    }
    crate::engine::append_to_system_prompt(&mut plan.argv, &sources_block);

    // project-container: `php` / `composer` of the project run inside the running docker
    // container that mounts the repo (a shim in `<run_dir>/bin`, first on PATH). Not for a dsh
    // engine (already in a container) or a human step.
    let in_container_engine = match &resolved.engine {
        crate::engine::Engine::Named(id) => ctx
            .cfg
            .engines
            .get(id)
            .is_some_and(|d| matches!(d.spec, crate::config::EngineSpec::Dsh(_))),
        crate::engine::Engine::Human(_) => true,
        crate::engine::Engine::Claude => false,
    };
    let run_container = if in_container_engine || plan.role == "inspect" {
        crate::container::RunContainer::default()
    } else {
        crate::container::for_run(&map, project, root)
    };
    for w in &run_container.warnings {
        crate::term::emit(&format!("Cảnh báo: {w}"));
    }
    if let Some((info, cmds, target)) = &run_container.active {
        crate::engine::append_to_system_prompt(
            &mut plan.argv,
            &crate::container::prompt_block(info, cmds, target),
        );
    }

    // C1: the run's real limit and end time, and the engine's shell limit when configured.
    let deadline_ms = crate::db::now_ms() + plan.timeout.as_millis() as i64;
    if plan.role != "inspect" {
        let tool_timeout = match &resolved.engine {
            crate::engine::Engine::Named(id) => ctx
                .cfg
                .engines
                .get(id)
                .and_then(|d| d.tool_timeout_seconds),
            _ => None,
        };
        let block = deadline_prompt_block(plan.timeout, &local_hhmm(deadline_ms), tool_timeout);
        crate::engine::append_to_system_prompt(&mut plan.argv, &block);
    }

    let mut input_warnings = Vec::new();
    let mut task_inputs = Vec::new();
    if let Some(task_id) = plan.task_id {
        if let Ok(inputs) = crate::inputs::list_for_task(&ctx.conn, task_id) {
            if !inputs.is_empty() {
                let (block, warns) = crate::inputs::prompt_block(&inputs);
                for w in &warns {
                    crate::term::emit(w);
                }
                input_warnings.extend(warns);
                crate::engine::append_to_system_prompt(&mut plan.argv, &block);
                task_inputs = inputs;
            }
        }
    }
    // D1: every prompt text above may name the protocol files by placeholder.
    let protocol = crate::protocol_paths::paths(project, &plan.task_slug);
    crate::engine::render_protocol_paths(&mut plan.argv, &protocol);
    // ENG-10 / D-36 / D-42 backstop: an engine tied to a profile never runs on a project of
    // another one (SPK-10), whatever team the task row names — a hand-edited team included.
    if let crate::engine::Engine::Named(id) = &resolved.engine {
        if let Some(def) = ctx.cfg.engines.get(id) {
            if let Some(msg) =
                crate::engine::profile_refusal(def, &project.id, &project.profile, &plan.role)
            {
                return Err(BrainError::error(msg));
            }
        }
    }
    let base_env = crate::profile::build_env(
        &project.profile,
        &ctx.cfg.profiles,
        &crate::profile::current_env(),
    )?;
    let mut ep = crate::engine::plan_for(
        &ctx.cfg,
        &resolved,
        &plan.role,
        &plan.argv,
        &plan.model,
        base_env,
        plan.timeout,
        root,
    )?;
    // An agy read-only role is told which protocol file to answer with by placeholder.
    crate::engine::render_protocol_paths(&mut ep.argv, &protocol);
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

    if plan.role != "inspect" {
        let _ = std::fs::write(run_dir.join(DEADLINE_FILE), deadline_ms.to_string());
    }

    if let Some((info, cmds, target)) = &run_container.active {
        let bin = crate::container::write_shims(&run_dir, info, cmds, target).map_err(|e| {
            BrainError::error(format!("Không tạo được shim container {}: {e}", info.name))
        })?;
        crate::container::prepend_path(&mut ep.env, &bin);
        match target {
            crate::container::ContainerTarget::Exec => {
                crate::term::emit(&format!(
                    "[agent-brain] lệnh {} chạy trong container {} ({})",
                    cmds.join(", "),
                    info.name,
                    info.dest
                ));
            }
            crate::container::ContainerTarget::Ephemeral { worktree } => {
                crate::term::emit(&format!(
                    "[agent-brain] lệnh {} chạy trong container tạm từ image {} ({})",
                    cmds.join(", "),
                    info.image,
                    worktree.display()
                ));
            }
        }
    }

    if let Some((_, prompt)) = crate::engine::claude_argv_parts(&plan.argv) {
        let prompt_path = run_dir.join("prompt.md");
        let redacted = crate::secrets::redact_str(&prompt, &ep.redact);
        let _ = std::fs::write(&prompt_path, redacted);
    }

    if !task_inputs.is_empty() {
        let inputs_dir = run_dir.join("inputs");
        let _ = std::fs::create_dir_all(&inputs_dir);
        for inp in &task_inputs {
            let file_path = inputs_dir.join(&inp.name);
            let _ = std::fs::write(&file_path, &inp.content);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }

    let p_desc = crate::profile::describe_profile(&project.profile, &ctx.cfg.profiles);
    // D-40: an endpoint with no E-09 evidence runs, and the header says so.
    let unverified_mark = if ep.unverified {
        " · chưa kiểm E-09"
    } else {
        ""
    };
    let member_mark = if let Some(ref pick) = resolved.member {
        format!(" · {}", pick.member.display_name)
    } else {
        String::new()
    };
    crate::term::emit(&format!(
        "[agent-brain] {} · {} · profile {} ({}) · engine {} · model {}{}{}",
        plan.role, project.id, project.profile, p_desc, ep.display, ep.model_label, member_mark, unverified_mark
    ));

    let stderr_path = run_dir.join("stderr.txt");
    let stream_path = run_dir.join("stream.jsonl");
    let quota_stderr_path = stderr_path.clone();

    let req = crate::runner::RunRequest {
        argv: ep.argv.clone(),
        cwd: root.to_path_buf(),
        env: ep.env.clone(),
        stderr_path,
        stream_path,
        timeout: plan.timeout,
        kill_grace: Duration::from_secs(ctx.cfg.kill_grace_secs),
        stdout_file: ep.stdout_file,
        redact: ep.redact.clone(),
        container_stop: ep.dsh.clone(),
        finish_after_result: Some(after_result_grace(ep.stream)),
        env_watch,
        stall_after: ctx.cfg.stall_after(),
    };

    // An agy Dev is not behind claude's PreToolUse hook: every tool step it starts is checked
    // here with the same policy (`policy.json`, written by the Dev before the run). Lead and the
    // reviewers run agy read-only and are not checked.
    let agy_policy = if plan.role == "dev" && ep.stream == crate::engine::StreamFormat::Agy {
        let policy_path = run_dir.join("policy.json");
        let policy = crate::guard::load_policy(&policy_path).map_err(|e| {
            BrainError::error(format!(
                "Không nạp được quyền của plan cho Dev agy ({}): {e}",
                policy_path.display()
            ))
        })?;
        Some(policy)
    } else {
        None
    };

    struct Observer<'a> {
        ctx: &'a Ctx,
        plan: &'a RunPlan,
        stage: &'a RunStage,
        project_id: &'a str,
        profile: &'a str,
        run_dir: &'a Path,
        db_engine: String,
        model_label: String,
        unverified: bool,
        stream: crate::engine::StreamFormat,
        dsh: Option<crate::dsh::DshRun>,
        team: Option<String>,
        member_id: Option<String>,
        summary: Summary,
        run_id: Option<i64>,
        seq: i64,
        agy_policy: Option<crate::guard::Policy>,
        agy_checked_steps: std::collections::BTreeSet<i64>,
        violation: Option<String>,
        /// Short reason shown when the observer stops the run for `violation`.
        stop_reason: Option<String>,
        /// `(kind, details)` stored through `record_violation` for a kind the run seam detects
        /// itself (C4: `wrong_cwd`).
        violation_record: Option<(&'static str, serde_json::Value)>,
        /// The project root an agy run must report as `init.cwd` (C4).
        agy_root: Option<PathBuf>,
        source_warnings: Vec<String>,
        /// project-container: `{"name", "dest", "commands"}` for `metrics_json.container`.
        container: Option<serde_json::Value>,
    }

    impl Observer<'_> {
        /// Checks one agy tool step about to run against the run's policy; the first refusal is
        /// kept as the run's violation and stops the run (`stop_requested`).
        fn check_agy_tool(&mut self, value: &serde_json::Value) {
            if self.violation.is_some() {
                return;
            }
            let Some(policy) = self.agy_policy.as_ref() else {
                return;
            };
            let Some((idx, tool, params)) = crate::stream::agy_active_tool(value) else {
                return;
            };
            if let Some(i) = idx {
                if !self.agy_checked_steps.insert(i) {
                    return;
                }
            }
            let Some(decision) = crate::guard::agy_tool_decision(policy, &tool, &params) else {
                return;
            };
            if decision.allow {
                return;
            }
            let shown: String = serde_json::Value::Object(params)
                .to_string()
                .chars()
                .take(200)
                .collect();
            let msg = format!(
                "agy Dev dùng công cụ ngoài quyền của plan: {tool} {shown} — {}",
                decision.reason
            );
            crate::term::emit(&msg);
            self.violation = Some(msg);
            self.stop_reason = Some("agy Dev dùng công cụ ngoài quyền của plan".to_string());
        }

        /// C4: agy must run with the project root as its workspace. An `init.cwd` that differs
        /// from the root (after canonicalize) stops the run as a `wrong_cwd` violation.
        fn check_agy_init(&mut self, value: &serde_json::Value) {
            if self.violation.is_some() {
                return;
            }
            let Some(root) = self.agy_root.as_ref() else {
                return;
            };
            let Some(cwd) = crate::guard::agy_init_cwd_mismatch(value, root) else {
                return;
            };
            let msg = format!(
                "agy chạy ở thư mục {cwd}, không phải gốc dự án {} — dừng run.",
                root.display()
            );
            crate::term::emit(&msg);
            self.violation = Some(msg);
            self.stop_reason = Some("agy chạy sai thư mục làm việc".to_string());
            self.violation_record = Some((
                crate::guard::VIOLATION_WRONG_CWD,
                serde_json::json!({"cwd": cwd, "root": root.to_string_lossy()}),
            ));
        }
    }

    impl<'a> crate::runner::RunObserver for Observer<'a> {
        fn on_spawn(&mut self, pid: i32, pgid: i32, pid_start: u64) -> BrainResult<()> {
            let proj_row_id = crate::db::project_row_id(&self.ctx.conn, self.project_id)?;
            let new_run = crate::db::NewRun {
                task_id: self.plan.task_id,
                project_id: proj_row_id,
                role: self.plan.role.clone(),
                profile: self.profile.to_string(),
                model: Some(self.model_label.clone()),
                pid: pid as i64,
                pgid: pgid as i64,
                pid_start: pid_start as i64,
                base_commit: self.plan.base_commit.clone(),
                run_dir: Some(self.run_dir.to_string_lossy().to_string()),
            };
            let run_id =
                crate::db::insert_run_with_engine(&self.ctx.conn, &new_run, &self.db_engine)?;
            self.run_id = Some(run_id);
            if let Some(ref mid) = self.member_id {
                let _ = crate::db::set_run_member_id(&self.ctx.conn, run_id, mid);
            }
            if self.stage.stage.is_some() || self.stage.round.is_some() {
                crate::db::set_run_stage_round(
                    &self.ctx.conn,
                    run_id,
                    self.stage.stage.as_deref(),
                    self.stage.round,
                )?;
            }
            // One merge: the stage metrics plus the team (JSON null for a legacy task).
            let mut patch = match self.stage.metrics.as_ref().and_then(|m| m.as_object()) {
                Some(obj) => obj.clone(),
                None => serde_json::Map::new(),
            };
            patch.insert(
                "team".to_string(),
                match &self.team {
                    Some(t) => serde_json::Value::String(t.clone()),
                    None => serde_json::Value::Null,
                },
            );
            if self.unverified {
                patch.insert("unverified".to_string(), serde_json::Value::Bool(true));
            }
            if !self.source_warnings.is_empty() {
                patch.insert(
                    "source_warnings".to_string(),
                    serde_json::to_value(&self.source_warnings).unwrap_or(serde_json::Value::Null),
                );
            }
            if let Some(c) = &self.container {
                patch.insert("container".to_string(), c.clone());
            }
            // ENG-09: what a kill from another process needs to find the container-side
            // processes of this run. Written here, before any kill can arrive.
            if let Some(run) = &self.dsh {
                patch.insert(
                    "dsh".to_string(),
                    serde_json::to_value(run).unwrap_or(serde_json::Value::Null),
                );
            }
            crate::db::merge_run_metrics(
                &self.ctx.conn,
                run_id,
                &serde_json::Value::Object(patch),
            )?;
            if let Some(tid) = self.plan.task_id {
                crate::term::emit(&format!("task {tid}, run {run_id}"));
            } else {
                crate::term::emit(&format!("run {run_id}"));
            }
            Ok(())
        }

        fn on_line(&mut self, line: &[u8]) {
            // D-35: dsh prints plain text. Each non-blank line becomes one `dsh/text` event so the
            // terminal, the database and the dashboard see it as text, never as a broken JSON line.
            let wrapped;
            let line = if self.stream == crate::engine::StreamFormat::Text {
                let text = String::from_utf8_lossy(line);
                let text = text.trim_end_matches(['\n', '\r']);
                if text.trim().is_empty() {
                    return;
                }
                wrapped = crate::dsh::text_event(text);
                wrapped.as_bytes()
            } else {
                line
            };
            if let Some(ev) = self.summary.feed(line) {
                if ev.kind == "agy/step_update" {
                    if let Some(value) = ev.value.as_ref() {
                        self.check_agy_tool(value);
                    }
                } else if ev.kind == "agy/init" {
                    if let Some(value) = ev.value.as_ref() {
                        self.check_agy_init(value);
                    }
                }
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

        fn result_seen(&self) -> bool {
            self.summary.result_seen()
        }

        fn result_is_error(&self) -> bool {
            self.summary.result_is_error()
        }

        fn stop_requested(&self) -> Option<String> {
            self.violation.as_ref().map(|_| {
                self.stop_reason
                    .clone()
                    .unwrap_or_else(|| "vi phạm giao thức".to_string())
            })
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
        db_engine: ep.db_engine.clone(),
        model_label: ep.model_label.clone(),
        unverified: ep.unverified,
        stream: ep.stream,
        dsh: ep.dsh.clone(),
        team: resolved.team.clone(),
        member_id: resolved.member.as_ref().map(|m| m.member.id.clone()),
        summary: Summary::new(plan.plan_files.clone(), Some(root.to_path_buf())),
        run_id: None,
        seq: 0,
        agy_policy,
        agy_checked_steps: std::collections::BTreeSet::new(),
        violation: None,
        stop_reason: None,
        violation_record: None,
        agy_root: (ep.stream == crate::engine::StreamFormat::Agy).then(|| root.to_path_buf()),
        source_warnings: source_warnings.clone(),
        container: run_container.metrics(),
    };

    let mut outcome = crate::runner::supervise(&req, &mut obs)?;
    obs.summary.finish();

    // The run was stopped for silence (`run_stall_minutes`); the status stays `timed_out` unless a
    // later rule below turns it into a success (agy answered first).
    let stall_error = outcome
        .stalled_after
        .filter(|_| outcome.stopped_by == Some(crate::runner::RunStop::Stalled))
        .map(crate::runner::stall_message);

    // C2: the environment went down while the run ran (`preflight_interval_seconds`).
    let env_down = outcome
        .env_down
        .clone()
        .filter(|_| outcome.stopped_by == Some(crate::runner::RunStop::EnvDown));

    // An agy Dev tool step outside the plan's permissions: the run was stopped for it (or ended
    // before the stop). Like every other protocol violation the run is a finished run with outcome
    // `protocol_violation` (what the autopilot stops on), not a kill by the user. A signal or a
    // timeout that came first keeps its own status.
    let violation = obs.violation.clone().filter(|_| match outcome.stopped_by {
        Some(crate::runner::RunStop::Observer) => true,
        None => outcome.status != crate::runner::RunStatus::Killed,
        Some(_) => false,
    });
    if violation.is_some() {
        outcome.status = crate::runner::RunStatus::Succeeded;
    }

    // agy exits 0 with a non-SUCCESS result more often than not (S7): the result decides. A
    // `--print-timeout` expiry looks like a success too (status SUCCESS, exit 0, empty response,
    // S7b); its only signal is a stderr line, so that line is what maps it to `timed_out`.
    let agy_stream = ep.stream == crate::engine::StreamFormat::Agy && violation.is_none();
    let print_timeout =
        agy_stream && agy_print_timeout_hit(&crate::quota::stderr_tail(&quota_stderr_path, 4096));
    // The run was stopped by the runner after agy had answered (it did not exit by itself): what
    // the role code sees is a run that ended by itself, the stored exit code stays the real one.
    let mut ended_after_answer = outcome.status == crate::runner::RunStatus::Succeeded
        && outcome.stopped_by == Some(crate::runner::RunStop::AfterResult);
    // A timeout that came after a complete SUCCESS answer loses nothing: the answer is used as if
    // agy had exited. A `--print-timeout` expiry (empty partial answer) stays a timeout.
    if agy_stream
        && outcome.status == crate::runner::RunStatus::TimedOut
        && !print_timeout
        && obs.summary.result_seen()
        && obs.summary.agy_status.as_deref() == Some("SUCCESS")
        && obs
            .summary
            .agy_response
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty())
    {
        outcome.status = crate::runner::RunStatus::Succeeded;
        ended_after_answer = true;
        crate::term::emit(
            "Run quá thời gian nhưng agy đã trả kết quả SUCCESS trước đó — dùng kết quả này",
        );
    }
    if agy_stream && outcome.status == crate::runner::RunStatus::Succeeded && print_timeout {
        outcome.status = crate::runner::RunStatus::TimedOut;
        ended_after_answer = false;
    }
    if agy_stream
        && outcome.status == crate::runner::RunStatus::Succeeded
        && obs.summary.result_seen()
        && obs.summary.agy_status.as_deref() != Some("SUCCESS")
    {
        outcome.status = crate::runner::RunStatus::Failed;
    }

    // ENG-02 / D-15: one detector decides quota for every claude-format run (claude, deepseek)
    // that was not killed or timed out. A hit only stops the step (D-16).
    let quota_hit = if violation.is_some() || env_down.is_some() {
        None
    } else if ep.stream == crate::engine::StreamFormat::Claude
        && !matches!(
            outcome.status,
            crate::runner::RunStatus::Killed | crate::runner::RunStatus::TimedOut
        )
    {
        crate::quota::detect_claude(
            &obs.summary,
            outcome.exit_code,
            &crate::quota::stderr_tail(&quota_stderr_path, 4096),
        )
    } else if agy_stream
        && !matches!(
            outcome.status,
            crate::runner::RunStatus::Killed | crate::runner::RunStatus::TimedOut
        )
    {
        crate::quota::detect_agy(
            obs.summary.agy_status.as_deref(),
            obs.summary.agy_error.as_deref(),
            outcome.exit_code,
            &crate::quota::stderr_tail(&quota_stderr_path, 4096),
        )
    } else if ep.stream == crate::engine::StreamFormat::Text
        && !matches!(
            outcome.status,
            crate::runner::RunStatus::Killed | crate::runner::RunStatus::TimedOut
        )
    {
        // dsh: text only, so a quota stop needs a non-zero exit (never an exit-0 answer).
        crate::quota::detect_text(
            outcome.exit_code,
            obs.summary.result_text.as_deref().unwrap_or(""),
            &crate::quota::stderr_tail(&quota_stderr_path, 4096),
        )
    } else {
        None
    };

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
            // claude (and a native entry) keeps the cost from its result event; other engines are
            // priced from `pricing.<engine>` and cost 0 when unpriced (D-23).
            cost_usd: crate::pricing::cost_for_engine(
                &ep.db_engine,
                &obs.summary.usage,
                &ctx.cfg.pricing,
            )
            .unwrap_or(if ep.native { obs.summary.cost_usd } else { 0.0 }),
            num_turns: obs.summary.num_turns,
            assistant_turns: Some(obs.summary.assistant_turns),
            turns_before_first_edit: obs.summary.turns_before_first_edit,
            reads_outside_plan: obs
                .summary
                .reads_outside_plan
                .as_ref()
                .map(|v| v.len() as i64),
            permission_denials: obs.summary.permission_denials,
            // Built from the already-redacted summary and stderr tail; redacted once more as a
            // defence so no later change to the detector can store a key here (ENG-05).
            error: match (&violation, &quota_hit) {
                (Some(v), _) => Some(crate::secrets::redact_str(v, &ep.redact)),
                (None, Some(h)) => Some(crate::secrets::redact_str(
                    &format!("quota: {}", h.detail),
                    &ep.redact,
                )),
                (None, None) => env_down
                    .as_ref()
                    .map(|r| crate::secrets::redact_str(&env_down_message(r), &ep.redact))
                    .or_else(|| {
                        stall_error
                            .clone()
                            .filter(|_| outcome.status == crate::runner::RunStatus::TimedOut)
                    }),
            },
        };
        crate::db::finish_run(&ctx.conn, run_id, &finish)?;
        if let Some((info, _, crate::container::ContainerTarget::Ephemeral { .. })) = &run_container.active {
            let run_tag = run_dir.file_name().and_then(|f| f.to_str()).unwrap_or("");
            crate::container::cleanup_ephemeral_containers(Some(&info.docker_bin), run_tag);
        }
        if ep.stream == crate::engine::StreamFormat::Agy {
            crate::db::merge_run_metrics(
                &ctx.conn,
                run_id,
                &serde_json::json!({"thinking_tokens": obs.summary.thinking_tokens}),
            )?;
            // The permission_mode agy reports is compared with what the probe showed for the
            // flag used (S7b). A difference is stored and printed, never silently accepted.
            let expected = crate::engine::agy_expected_permission_mode(&plan.role);
            if let Some(seen) = obs.summary.agy_permission_mode.as_deref() {
                if seen != expected {
                    crate::db::merge_run_metrics(
                        &ctx.conn,
                        run_id,
                        &serde_json::json!({"agy": {"permission_mode": seen, "expected": expected}}),
                    )?;
                    crate::term::emit(&format!(
                        "Cảnh báo: agy chạy với permission_mode {seen}, mong đợi {expected}"
                    ));
                }
            }
        }
        crate::term::emit(&crate::term::summary_line(
            run_id,
            plan.task_id,
            outcome.status.as_str(),
            &obs.summary,
            outcome.elapsed,
        ));
        if let Some(hit) = &quota_hit {
            crate::db::set_run_outcome(&ctx.conn, run_id, crate::quota::OUTCOME_QUOTA)?;
            crate::db::merge_run_metrics(
                &ctx.conn,
                run_id,
                &serde_json::json!({
                    "quota": {
                        "engine": ep.display,
                        "signal": hit.signal,
                        "reset_at": hit.reset_at_ms,
                    }
                }),
            )?;
        }
    }

    // Same exit as the other Dev protocol violations: nothing is committed or undone.
    if let Some(msg) = violation {
        if let Some(run_id) = obs.run_id {
            crate::db::set_run_outcome(&ctx.conn, run_id, "protocol_violation")?;
            if let Some((kind, details)) = obs.violation_record.take() {
                record_violation(
                    ctx,
                    run_id,
                    kind,
                    &crate::secrets::redact_str(&msg, &ep.redact),
                    details,
                )?;
            }
        }
        return Err(BrainError::new(
            crate::secrets::redact_str(
                &format!(
                    "{msg}\nRun đã bị dừng — vi phạm giao thức. agent-brain không tự hoàn tác."
                ),
                &ep.redact,
            ),
            EXIT_PROTOCOL,
        ));
    }

    // C2: like a quota stop, the role code never runs after an environment stop.
    if let Some(reason) = env_down {
        if let Some(run_id) = obs.run_id {
            crate::db::set_run_outcome(&ctx.conn, run_id, crate::preflight::OUTCOME_ENV_DOWN)?;
        }
        return Err(BrainError::new(
            crate::secrets::redact_str(&env_down_message(&reason), &ep.redact),
            EXIT_ENV_DOWN,
        ));
    }

    // The role code after this call never runs on a quota stop: nothing is committed, nothing
    // retries and no other engine is tried. `main` prints the message on stderr.
    if let Some(hit) = quota_hit {
        return Err(BrainError::new(
            crate::secrets::redact_str(&crate::quota::user_message(&ep.display, &hit), &ep.redact),
            EXIT_QUOTA,
        ));
    }

    // ENG-05 / Pitfall 10: redaction covers what agent-brain writes, not what the model writes into
    // the project. After a run that ended by itself, the files it changed (uncommitted, plus
    // committed since the run's base) are scanned for this run's key BEFORE the role code gets to
    // commit anything. A hit stops the step; only the paths are reported, never the value.
    if !ep.redact.is_empty()
        && !matches!(
            outcome.status,
            crate::runner::RunStatus::Killed | crate::runner::RunStatus::TimedOut
        )
    {
        let mut paths = crate::gitops::changed_paths(root).unwrap_or_default();
        if let Some(base) = plan.base_commit.as_deref() {
            if let Ok(names) = crate::gitops::diff_names(root, base, "HEAD") {
                paths.extend(names);
            }
        }
        let paths: Vec<String> = paths.into_iter().collect();
        let leaked = crate::secrets::scan_for_secrets(root, &paths, &ep.redact);
        if !leaked.is_empty() {
            let msg = format!(
                "Phát hiện key của engine {} trong file: {} — không commit. Xóa key khỏi file (và khỏi lịch sử git nếu đã commit) rồi chạy lại.",
                ep.db_engine,
                leaked.join(", ")
            );
            let redacted_msg = crate::secrets::redact_str(&msg, &ep.redact);
            if let Some(run_id) = obs.run_id {
                crate::db::set_run_outcome(&ctx.conn, run_id, "protocol_violation")?;
                crate::db::set_run_error(&ctx.conn, run_id, &redacted_msg)?;
                crate::db::merge_run_metrics(
                    &ctx.conn,
                    run_id,
                    &serde_json::json!({
                        "secret_leak": leaked,
                        "violation": {
                            "kind": "secret_leak",
                            "files": leaked
                        }
                    }),
                )?;
            }
            return Err(BrainError::new(
                redacted_msg,
                EXIT_PROTOCOL,
            ));
        }
    }

    // D-05: a read-only agy role answers with the whole protocol file; agent-brain writes it, and
    // the role's own checks (meta, plan validation, review front matter) decide what it is worth.
    if agy_stream && outcome.status == crate::runner::RunStatus::Succeeded && plan.role != "analyst" && plan.role != "analysis-review" {
        if let (Some(file), Some(resp)) = (ep.response_file, obs.summary.agy_response.as_deref()) {
            if obs.summary.agy_status.as_deref() == Some("SUCCESS") && !resp.trim().is_empty() {
                let mut body = unwrap_code_fence(resp);
                if !body.ends_with('\n') {
                    body.push('\n');
                }
                protocol.ensure_dir(root)?;
                std::fs::write(root.join(protocol.get(file)), body)?;
            }
        }
    }

    if ended_after_answer && outcome.status == crate::runner::RunStatus::Succeeded {
        // The stop signal's exit code (143/137) is in `runs.exit_code`; the role code must not
        // read it as a failure of a run that had already answered.
        outcome.exit_code = Some(0);
    }

    let run_id = obs.run_id.unwrap_or(0);
    let mut all_warnings = source_warnings;
    all_warnings.extend(input_warnings);
    all_warnings.extend(run_container.warnings.iter().cloned());
    if run_id > 0 && !all_warnings.is_empty() {
        let _ = crate::db::merge_run_metrics(
            &ctx.conn,
            run_id,
            &serde_json::json!({ "warnings": all_warnings }),
        );
    }
    Ok(RoleRun {
        run_id,
        outcome,
        summary: obs.summary,
        run_dir,
        warnings: all_warnings,
    })
}

pub fn record_violation(
    ctx: &crate::app::Ctx,
    run_id: i64,
    kind: &str,
    message: &str,
    details: serde_json::Value,
) -> BrainResult<()> {
    let mut secrets = crate::inputs::env_secrets();
    for key_env in ctx.cfg.engines.all_key_envs() {
        if let Ok(v) = std::env::var(key_env) {
            if v.len() >= crate::secrets::MIN_SECRET_BYTES {
                secrets.push(v);
            }
        }
    }
    let redacted = crate::secrets::redact_str(message, &secrets);
    crate::db::set_run_error(&ctx.conn, run_id, &redacted)?;

    let mut violation_obj = match details {
        serde_json::Value::Object(m) => m,
        other => {
            let mut m = serde_json::Map::new();
            m.insert("details".to_string(), other);
            m
        }
    };
    violation_obj.insert("kind".to_string(), serde_json::json!(kind));

    crate::db::merge_run_metrics(
        &ctx.conn,
        run_id,
        &serde_json::json!({ "violation": serde_json::Value::Object(violation_obj) }),
    )?;
    Ok(())
}

/// True when agy's stderr tail carries the `--print-timeout` expiry line:
/// `[agy] print timeout after Ns with turn in progress; returning partial output`.
fn agy_print_timeout_hit(stderr_tail: &str) -> bool {
    stderr_tail
        .lines()
        .any(|l| l.contains("print timeout after") && l.contains("returning partial output"))
}

/// Removes one outer code-fence wrapper (first line starts with three backticks and the last
/// non-empty line is three backticks); any other text is returned unchanged.
pub(crate) fn unwrap_code_fence(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (lines.first(), last) {
        (Some(first), Some(end))
            if end > 0 && first.trim_start().starts_with("```") && lines[end].trim() == "```" =>
        {
            lines[1..end].join("\n")
        }
        _ => text.to_string(),
    }
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

    if opts.finalize && role != Role::Dev {
        return Err(BrainError::error("--finalize chỉ dùng với vai trò dev"));
    }

    if (opts.review
        || (opts.note.is_some() && !(role == Role::Dev && opts.finalize))
        || (opts.workflow.is_some() && role != Role::Analyst))
        && role != Role::Lead
    {
        return Err(BrainError::error(
            "--review, --note và --workflow chỉ dùng với vai trò lead",
        ));
    }

    if opts.origin.is_some() && role != Role::Lead && role != Role::Analyst {
        return Err(BrainError::error(
            "--origin chỉ dùng với vai trò lead hoặc analyst",
        ));
    }

    if opts.team.is_some() && role != Role::Lead && role != Role::Analyst {
        return Err(BrainError::error(
            "--team chỉ dùng với vai trò lead hoặc analyst",
        ));
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
                        "Bỏ qua <yêu cầu> cho vai trò dev: vai trò này chỉ đọc file plan / git diff",
                    );
                }
            }
            dev::cmd_dev(ctx, project, opts)
        }
        Role::Qa => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(
                        "Bỏ qua <yêu cầu> cho vai trò qa: vai trò này chỉ đọc file plan / git diff",
                    );
                }
            }
            qa::cmd_qa(ctx, project, opts)
        }
        Role::PlanReview => {
            if let Some(r) = request {
                if !r.trim().is_empty() {
                    crate::term::emit(&format!(
                        "Bỏ qua <yêu cầu> cho vai trò {}: vai trò này chỉ đọc file plan / git diff",
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
                        "Bỏ qua <yêu cầu> cho vai trò {}: vai trò này chỉ đọc file plan / git diff",
                        role.as_str()
                    ));
                }
            }
            review::cmd_review(ctx, project, crate::report::ReviewKind::Code, opts)
        }
        Role::Analyst => {
            let req = match request {
                Some(r) if !r.trim().is_empty() => r.trim(),
                _ => {
                    return Err(BrainError::error(
                        "run analyst cần <câu hỏi> — ví dụ: agent-brain run analyst \"<dự án>\" \"<câu hỏi>\"",
                    ));
                }
            };
            analyst::cmd_analyst(ctx, project, req, opts)
        }
        Role::AnalysisReview => {
            Err(BrainError::error(
                "Vai trò analysis-review chỉ được gọi nội bộ bởi bước review của phân tích",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_block_uses_the_real_numbers() {
        let b = deadline_prompt_block(Duration::from_secs(2700), "14:05", Some(120));
        assert!(b.contains(
            "Bạn có 45 phút cho lần chạy này (hết lúc 14:05). Hãy commit hoặc ghi kết quả trước khi hết giờ."
        ));
        assert!(b.contains(
            "Mỗi lệnh shell bị cắt sau 2 phút. Lệnh dài hơn phải chia nhỏ, không chạy nền."
        ));
        let odd = deadline_prompt_block(Duration::from_secs(90), "09:00", Some(45));
        assert!(odd.contains("Bạn có 1 phút 30 giây"));
        assert!(odd.contains("bị cắt sau 45 giây"));
    }

    #[test]
    fn deadline_block_has_no_shell_line_without_tool_timeout() {
        let b = deadline_prompt_block(Duration::from_secs(1800), "10:30", None);
        assert!(b.contains("Bạn có 30 phút cho lần chạy này (hết lúc 10:30)."));
        assert!(!b.contains("lệnh shell"));
    }

    #[test]
    fn local_hhmm_is_hours_and_minutes() {
        let t = local_hhmm(1_700_000_000_000);
        assert_eq!(t.len(), 5);
        assert_eq!(&t[2..3], ":");
    }

    #[test]
    fn agy_print_timeout_line_detected() {
        assert!(agy_print_timeout_hit(
            "[agy] print timeout after 5s with turn in progress; returning partial output\n"
        ));
        assert!(!agy_print_timeout_hit("error: context canceled\n"));
        assert!(!agy_print_timeout_hit(""));
    }

    #[test]
    fn unwrap_code_fence_rules() {
        assert_eq!(unwrap_code_fence("```markdown\nA\nB\n```\n"), "A\nB");
        assert_eq!(unwrap_code_fence("```\nA\n```"), "A");
        assert_eq!(unwrap_code_fence("A\n```x\nB\n```"), "A\n```x\nB\n```");
        assert_eq!(unwrap_code_fence("---\nmeta\n---\nA"), "---\nmeta\n---\nA");
        assert_eq!(unwrap_code_fence("```"), "```");
        assert_eq!(unwrap_code_fence(""), "");
    }

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

    #[test]
    fn test_record_violation_stores_error_and_metrics() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        let project = crate::projects::Project {
            id: "p".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            name_vi: "P".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            hidden: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        };
        crate::db::sync_projects(&conn, &[project]).unwrap();
        let pid = crate::db::project_row_id(&conn, "p").unwrap();
        let tid = crate::db::upsert_task(&conn, pid, "t", "req", "branch", None, None).unwrap();
        let new_run = crate::db::NewRun {
            task_id: Some(tid),
            project_id: pid,
            role: "lead".to_string(),
            profile: "default".to_string(),
            model: None,
            pid: 1,
            pgid: 1,
            pid_start: 1,
            base_commit: None,
            run_dir: None,
        };
        let run_id = crate::db::insert_run(&conn, &new_run).unwrap();

        let cfg = crate::config::config_from_value(
            &serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap(),
        )
        .unwrap();
        let ctx = crate::app::Ctx {
            conn,
            cfg,
            map: serde_json::json!({}),
            projects: Vec::new(),
        };

        record_violation(
            &ctx,
            run_id,
            "missing_plan",
            "Không tìm thấy file .plan.md",
            serde_json::json!({ "checked_paths": [".plan.md"] }),
        )
        .unwrap();

        let row = crate::db::get_run(&ctx.conn, run_id).unwrap().unwrap();
        assert_eq!(row.error.as_deref(), Some("Không tìm thấy file .plan.md"));
        let metrics: serde_json::Value =
            serde_json::from_str(row.metrics_json.as_deref().unwrap()).unwrap();
        assert_eq!(metrics["violation"]["kind"], "missing_plan");
        assert_eq!(metrics["violation"]["checked_paths"][0], ".plan.md");
    }

    #[test]
    fn test_task_66_replan_scenario_code_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let init = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(init.status.success());

        let do_commit = |files: &[(&str, &str)], msg: &str| -> String {
            for (rel, content) in files {
                let p = root.join(rel);
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::write(&p, content).unwrap();
            }
            let add = std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(root)
                .output()
                .unwrap();
            assert!(add.status.success());
            let commit = std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    msg,
                ])
                .current_dir(root)
                .output()
                .unwrap();
            assert!(commit.status.success());
            crate::gitops::head_sha(root).unwrap()
        };

        let base = do_commit(&[("README.md", "hello")], "init");

        let checkout = std::process::Command::new("git")
            .args(["checkout", "-b", "task/t-66"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(checkout.status.success());

        // 1. Plan 1
        let p1 = do_commit(&[(".plan.md", "v1")], "t-66: plan");

        // Set up brain.db
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        let project = crate::projects::Project {
            id: "p".to_string(),
            path: root.to_string_lossy().to_string(),
            name_vi: "P".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            hidden: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
            workspace: Default::default(),
        };
        crate::db::sync_projects(&conn, &[project.clone()]).unwrap();
        let pid = crate::db::project_row_id(&conn, "p").unwrap();
        let _tid = crate::db::upsert_task(
            &conn,
            pid,
            "t-66",
            "req",
            "task/t-66",
            Some(&base),
            None,
        )
        .unwrap();

        // Case A: No code after Plan 1
        let anchor_a = code_anchor_for(&conn, "p", root, "t-66").unwrap().unwrap();
        assert_eq!(anchor_a, p1);
        let diff_a = crate::gitops::diff_code_names(root, &anchor_a, "HEAD").unwrap();
        assert!(diff_a.is_empty(), "No code yet after plan 1");

        // 2. Dev commits code
        do_commit(&[("src/App.php", "<?php echo 1;")], "t-66: add feature");

        // 3. Lead replans (Plan 2)
        let p2 = do_commit(&[(".plan.md", "v2")], "t-66: plan");

        // Case B: Task 66 scenario — base -> plan 1 -> code -> plan 2 -> HEAD
        // plan_commit_for returns newest plan (p2)
        let latest_plan = plan_commit_for(&conn, "p", root, "t-66").unwrap().unwrap();
        assert_eq!(latest_plan, p2);

        // code diff from latest_plan to HEAD is empty (HEAD is plan 2)
        let diff_latest = crate::gitops::diff_code_names(root, &latest_plan, "HEAD").unwrap();
        assert!(diff_latest.is_empty(), "latest plan to HEAD has no code");

        // code_anchor_for returns oldest plan (p1)
        let anchor_b = code_anchor_for(&conn, "p", root, "t-66").unwrap().unwrap();
        assert_eq!(anchor_b, p1);

        // code diff from anchor_b to HEAD contains src/App.php
        let diff_b = crate::gitops::diff_code_names(root, &anchor_b, "HEAD").unwrap();
        assert!(diff_b.contains("src/App.php"));
        assert!(!diff_b.is_empty(), "code_after_plan must be true");

        // Check protocol code_diff_command uses anchor
        let protocol = crate::protocol_paths::paths(&project, "t-66");
        let diff_cmd = protocol.code_diff_command(&anchor_b);
        assert!(diff_cmd.contains(&p1));
    }
}
