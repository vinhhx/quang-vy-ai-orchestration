//! Danh mục engine (D-30): `claude` và `người` có sẵn, còn lại là danh sách `engines` của config.json.
//! Lệnh, endpoint và biến key chỉ nằm trong config.json; ở đây chỉ có kiểu, cách dựng lệnh chạy và độ sẵn sàng tính tại máy.

use crate::config::{AgyDef, AnthropicDef, Config, EngineDef, EngineSpec};
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

pub const ENGINE_CLAUDE: &str = "claude";
/// Default catalog ids (used when `engines` is absent or in the legacy object shape).
pub const ENGINE_DEEPSEEK: &str = "deepseek";
pub const ENGINE_AGY: &str = "agy";
/// `runs.engine` of a human run (NFC).
pub const ENGINE_HUMAN: &str = "người";

const HUMAN_PREFIX: &str = "người:";
/// Longest engine id a config entry may have.
pub const MAX_ENGINE_ID_BYTES: usize = 32;

/// Ids in display order: `claude`, the configured ids in file order, then `người`.
pub fn engine_order(cfg: &Config) -> Vec<String> {
    let mut order = vec![ENGINE_CLAUDE.to_string()];
    order.extend(cfg.engines.list.iter().map(|e| e.id.clone()));
    order.push(ENGINE_HUMAN.to_string());
    order
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Engine {
    Claude,
    /// The id of a configured engine (`config.json` `engines`).
    Named(String),
    Human(String),
}

impl Engine {
    /// Trim + NFC, then `claude` | `<engine id>` | `người:<id>` with a valid staff id. Whether a
    /// named id exists in the engine list is decided at the run seam, not here (D-43).
    pub fn parse(s: &str) -> Option<Engine> {
        let normalized: String = s.trim().nfc().collect();
        if normalized == ENGINE_CLAUDE {
            return Some(Engine::Claude);
        }
        if let Some(id) = normalized.strip_prefix(HUMAN_PREFIX) {
            return if crate::staff::validate_id(id) {
                Some(Engine::Human(id.to_string()))
            } else {
                None
            };
        }
        if crate::workflows::validate_name(&normalized) && normalized.len() <= MAX_ENGINE_ID_BYTES {
            Some(Engine::Named(normalized))
        } else {
            None
        }
    }

    /// The value stored in `runs.engine`.
    pub fn as_db_str(&self) -> &str {
        match self {
            Engine::Claude => ENGINE_CLAUDE,
            Engine::Named(id) => id,
            Engine::Human(_) => ENGINE_HUMAN,
        }
    }

    /// The spelling used in teams.json.
    pub fn to_config_string(&self) -> String {
        match self {
            Engine::Human(id) => format!("{HUMAN_PREFIX}{id}"),
            other => other.as_db_str().to_string(),
        }
    }

    pub fn is_human(&self) -> bool {
        matches!(self, Engine::Human(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    SanSang,
    ThieuKey,
    ThieuLenh,
    ChuaKiem,
}

impl Readiness {
    pub fn key(&self) -> &'static str {
        match self {
            Readiness::SanSang => "san_sang",
            Readiness::ThieuKey => "thieu_key",
            Readiness::ThieuLenh => "thieu_lenh",
            Readiness::ChuaKiem => "chua_kiem",
        }
    }

    pub fn word(&self) -> &'static str {
        match self {
            Readiness::SanSang => "sẵn sàng",
            Readiness::ThieuKey => "thiếu key",
            Readiness::ThieuLenh => "thiếu lệnh",
            Readiness::ChuaKiem => "chưa kiểm",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EngineStatus {
    pub name: String,
    pub readiness: &'static str,
    pub word: &'static str,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub key_env: Option<String>,
    pub model: Option<String>,
    pub description: &'static str,
    pub detail: Option<String>,
    /// `claude` | `anthropic` | `agy` | `dsh` | `người` (D-30).
    pub kind: String,
    /// The entry's label from config.json.
    pub label: Option<String>,
    /// The user's `verified` text for an `anthropic` entry (the E-09 evidence, D-40).
    pub evidence: Option<String>,
    /// `tokens` when runs report token usage, `none` for a black-box engine (dsh) and for `người`.
    pub usage: &'static str,
    pub pin_profile: Option<String>,
    pub allowed_profiles: Option<Vec<String>>,
    /// The container name of a `dsh` entry.
    pub container: Option<String>,
}

fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Local lookup only: a value with `/` must be an executable file; otherwise each
/// `PATH` entry is searched. An empty or missing `PATH` finds nothing.
pub fn find_command(cmd: &str, path_var: Option<&str>) -> bool {
    if cmd.is_empty() {
        return false;
    }
    if cmd.contains('/') {
        return is_executable_file(Path::new(cmd));
    }
    let Some(path_var) = path_var else {
        return false;
    };
    path_var
        .split(':')
        .filter(|dir| !dir.is_empty())
        .any(|dir| is_executable_file(&Path::new(dir).join(cmd)))
}

const DESC_CLAUDE: &str = "Dùng profile của dự án (work hoặc personal), không có tài khoản riêng.";
const DESC_DEEPSEEK: &str = "claude chạy trên máy với endpoint của DeepSeek.";
const DESC_AGY: &str = "Antigravity chạy không giao diện (headless).";
const DESC_ANTHROPIC: &str = "claude chạy trên máy với một endpoint định dạng Anthropic.";
const DESC_NATIVE: &str = "claude chạy trên máy theo profile của dự án (không đổi endpoint).";
const DESC_DSH: &str = "DeepSeek Harness chạy trong container (hộp đen, chỉ văn bản).";
const DESC_HUMAN: &str = "Nhân sự làm trên máy của họ và đẩy nhánh task lên remote.";

const REASON_UNVERIFIED: &str = "Endpoint chưa có bằng chứng E-09 — chạy spikes/e09/run.sh rồi ghi \"verified\" cho engine này trong config.json.";
const DSH_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

fn command_status(
    name: &str,
    kind: &str,
    command: &str,
    path_var: Option<&str>,
    description: &'static str,
) -> EngineStatus {
    let found = find_command(command, path_var);
    let readiness = if found {
        Readiness::SanSang
    } else {
        Readiness::ThieuLenh
    };
    EngineStatus {
        name: name.to_string(),
        readiness: readiness.key(),
        word: readiness.word(),
        reason: (!found).then(|| format!("Không tìm thấy lệnh {command} trên máy.")),
        command: Some(command.to_string()),
        key_env: None,
        model: None,
        description,
        detail: None,
        kind: kind.to_string(),
        label: None,
        evidence: None,
        usage: "tokens",
        pin_profile: None,
        allowed_profiles: None,
        container: None,
    }
}

fn set_readiness(st: &mut EngineStatus, r: Readiness, reason: Option<String>) {
    st.readiness = r.key();
    st.word = r.word();
    st.reason = reason;
}

/// Container status text from `{docker} ps --filter name=^{container}$ --format {{.Status}}`:
/// stdin null, stderr discarded, at most 3 s, read-only. `None` on any failure. Only the child
/// handle spawned here is ever killed.
pub fn docker_status_probe(docker: &str, container: &str) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(docker)
        .args(["ps", "--filter"])
        .arg(format!("name=^{container}$"))
        .args(["--format", "{{.Status}}"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + DSH_PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    out.lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

/// Readiness of every engine, in `engine_order`, from local checks only: a PATH lookup, whether
/// the key variable is set, the user's `verified` text, and for `dsh` the container status
/// through `docker_status_probe`. No model call, and no env VALUE is ever copied into the
/// result (D-03).
pub fn readiness(
    cfg: &Config,
    env: &BTreeMap<String, String>,
    staff_count: usize,
) -> Vec<EngineStatus> {
    readiness_with(cfg, env, staff_count, &docker_status_probe)
}

/// `readiness` with the container probe injected: `probe(docker, container)` returns the status
/// text of the container, or `None` when it cannot be read. Tests never run a real docker.
pub fn readiness_with(
    cfg: &Config,
    env: &BTreeMap<String, String>,
    staff_count: usize,
    probe: &dyn Fn(&str, &str) -> Option<String>,
) -> Vec<EngineStatus> {
    let path_var = env.get("PATH").map(String::as_str);

    let mut out = vec![command_status(
        ENGINE_CLAUDE,
        ENGINE_CLAUDE,
        &cfg.claude_bin,
        path_var,
        DESC_CLAUDE,
    )];

    for def in &cfg.engines.list {
        out.push(entry_status(def, env, path_var, probe));
    }

    let mut human = command_status(ENGINE_HUMAN, ENGINE_HUMAN, "", path_var, DESC_HUMAN);
    set_readiness(&mut human, Readiness::SanSang, None);
    human.command = None;
    human.detail = Some(format!("{staff_count} nhân sự"));
    human.usage = "none";
    out.push(human);
    out
}

fn entry_status(
    def: &EngineDef,
    env: &BTreeMap<String, String>,
    path_var: Option<&str>,
    probe: &dyn Fn(&str, &str) -> Option<String>,
) -> EngineStatus {
    let kind = def.kind().as_str();
    let mut st = match &def.spec {
        EngineSpec::Anthropic(a) => {
            let description = if a.base_url.is_none() {
                DESC_NATIVE
            } else if def.id == ENGINE_DEEPSEEK {
                DESC_DEEPSEEK
            } else {
                DESC_ANTHROPIC
            };
            let mut st = command_status(&def.id, kind, &a.command, path_var, description);
            st.key_env = a.key_env.clone();
            st.evidence = a.verified.clone();
            st.pin_profile = a.pin_profile.clone();
            let mut models: Vec<&str> = Vec::new();
            for role in crate::teams::ROLES {
                if let Some(m) = a.models.get(role) {
                    if !models.contains(&m.as_str()) {
                        models.push(m);
                    }
                }
            }
            if !models.is_empty() {
                st.model = Some(models.join(", "));
            }
            // thiếu lệnh and thiếu key both win over chưa kiểm (D-40).
            if st.readiness == Readiness::SanSang.key() && a.base_url.is_some() {
                let key_unset = a
                    .key_env
                    .as_ref()
                    .filter(|k| !env.get(*k).map_or(false, |v| !v.trim().is_empty()));
                if let Some(key_env) = key_unset {
                    set_readiness(
                        &mut st,
                        Readiness::ThieuKey,
                        Some(format!("Chưa đặt biến môi trường {key_env}.")),
                    );
                } else if a.verified.is_none() {
                    set_readiness(
                        &mut st,
                        Readiness::ChuaKiem,
                        Some(REASON_UNVERIFIED.to_string()),
                    );
                }
            }
            st
        }
        EngineSpec::Agy(a) => {
            let mut st = command_status(&def.id, kind, &a.command, path_var, DESC_AGY);
            st.model = a.model.clone();
            st
        }
        EngineSpec::Dsh(d) => {
            let mut st = command_status(&def.id, kind, &d.docker, path_var, DESC_DSH);
            st.model = Some(d.dsh_profile.clone());
            st.container = Some(d.container.clone());
            st.usage = "none";
            if st.readiness == Readiness::SanSang.key() {
                let running = probe(&d.docker, &d.container)
                    .map_or(false, |status| status.trim_start().starts_with("Up"));
                if !running {
                    set_readiness(
                        &mut st,
                        Readiness::ThieuLenh,
                        Some(format!("Container {} chưa chạy.", d.container)),
                    );
                }
            }
            st
        }
    };
    st.label = def.label.clone();
    st.allowed_profiles = def.effective_allowed_profiles();
    st
}

// ---- the engine seam (D-14): one decision per role run ----

/// The engine a role run uses, decided from the task's stored team.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEngine {
    pub engine: Engine,
    /// `None` = a legacy task (no `team_json`) or a run without a task.
    pub team: Option<String>,
    pub wait: Option<crate::teams::HumanWait>,
}

impl ResolvedEngine {
    fn claude() -> ResolvedEngine {
        ResolvedEngine {
            engine: Engine::Claude,
            team: None,
            wait: None,
        }
    }
}

/// Task's stored team -> the role's engine. No task, or a task created before Phase 6 (NULL
/// `team_json`), runs on claude. A team that no longer exists refuses the step: the engine is
/// never switched silently (D-16). While the task waits for a person, only the waiting role runs.
pub fn resolve_for_run(
    conn: &rusqlite::Connection,
    task_id: Option<i64>,
    role: &str,
) -> BrainResult<ResolvedEngine> {
    let Some(task_id) = task_id else {
        return Ok(ResolvedEngine::claude());
    };
    let Some(raw) = crate::db::task_team_json(conn, task_id)? else {
        return Ok(ResolvedEngine::claude());
    };
    let task_team = crate::teams::TaskTeam::parse(Some(&raw)).ok_or_else(|| {
        BrainError::error(format!(
            "Không đọc được đội hình của task {task_id} — đổi đội hình rồi chạy lại."
        ))
    })?;
    let (loaded, staff) = crate::teams::load();
    let team = loaded.get(&task_team.name).ok_or_else(|| {
        BrainError::error(format!(
            "Đội hình '{}' của task không còn trong teams.json — đổi đội hình rồi chạy lại.",
            task_team.name
        ))
    })?;

    if let Some(wait) = &task_team.wait {
        if wait.role != role {
            let step = if wait.stage == "fix" { "Sửa" } else { "Code" };
            return Err(BrainError::error(format!(
                "Task đang chờ {} làm bước {step} — bấm Xong trên dashboard (hoặc agent-brain human done {task_id}) trước khi chạy bước khác.",
                staff.display_name_of(&wait.staff)
            )));
        }
    }

    Ok(ResolvedEngine {
        engine: team.engine_for(role),
        team: Some(task_team.name.clone()),
        wait: task_team.wait.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    Claude,
    Agy,
    /// Plain text (an engine with no token stream, e.g. dsh).
    Text,
}

/// What `execute_role_staged` spawns and records for one run.
#[derive(Debug, Clone)]
pub struct EnginePlan {
    pub engine: Engine,
    /// Value stored in `runs.engine`: the engine id.
    pub db_engine: String,
    /// Header word: `claude` | the engine id | `người:<id>`.
    pub display: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub model_label: String,
    pub stream: StreamFormat,
    pub stdout_file: bool,
    /// Secret values to scrub from anything written to disk.
    pub redact: Vec<String>,
    pub response_file: Option<&'static str>,
    /// Runs on the project's profile with nothing injected: `claude` and an `anthropic` entry
    /// without `base_url`. Such a run keeps the cost claude reports.
    pub native: bool,
    /// An `anthropic` entry with `base_url` and no `verified` text: it runs, but is marked (D-40).
    pub unverified: bool,
    /// The container run a dsh engine persists for kill and cleanup (plan 06-23).
    pub dsh: Option<crate::dsh::DshRun>,
}

/// Env variables an endpoint run never inherits: they would pick the account or the model instead
/// of the endpoint the entry names.
const ENDPOINT_STRIPPED_ENV: [&str; 6] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "CLAUDE_CODE_SUBAGENT_MODEL",
];

/// Removes the key variable of every configured engine from a child env (Pitfall A2): a child
/// sees at most its own engine's token, and that one is injected under a different name.
fn strip_key_envs(env: &mut BTreeMap<String, String>, cfg: &Config) {
    for name in cfg.engines.all_key_envs() {
        env.remove(&name);
    }
}

/// D-43: a team names an engine id that is not in the engine list. The id is never resolved to
/// another endpoint.
fn unknown_engine_error(id: &str, team: Option<&str>) -> BrainError {
    BrainError::error(format!(
        "Engine '{id}' của đội hình '{}' không có trong config.json (engines) — thêm engine hoặc đổi đội hình rồi chạy lại.",
        team.unwrap_or("-")
    ))
}

/// `claude_argv` is the claude-family argv the role built; `base_env` is the project profile's
/// env (D-07: the claude engine always uses it unchanged, minus every engine's key variable).
/// Every other engine derives its own argv from the claude one. Everything is decided here,
/// before any run row exists.
#[allow(clippy::too_many_arguments)]
pub fn plan_for(
    cfg: &Config,
    resolved: &ResolvedEngine,
    role: &str,
    claude_argv: &[String],
    claude_model: &str,
    base_env: BTreeMap<String, String>,
    timeout: Duration,
    cwd: &Path,
) -> BrainResult<EnginePlan> {
    match &resolved.engine {
        Engine::Claude => {
            let mut env = base_env;
            strip_key_envs(&mut env, cfg);
            Ok(EnginePlan {
                engine: Engine::Claude,
                db_engine: ENGINE_CLAUDE.to_string(),
                display: ENGINE_CLAUDE.to_string(),
                argv: claude_argv.to_vec(),
                env,
                model_label: claude_model.to_string(),
                stream: StreamFormat::Claude,
                stdout_file: false,
                redact: Vec::new(),
                response_file: None,
                native: true,
                unverified: false,
                dsh: None,
            })
        }
        Engine::Human(_) => Err(BrainError::error(
            "Engine người không chạy tiến trình — bước Dev của nhân sự bắt đầu bằng agent-brain run dev",
        )),
        Engine::Named(id) => {
            let def = cfg
                .engines
                .get(id)
                .ok_or_else(|| unknown_engine_error(id, resolved.team.as_deref()))?;
            match &def.spec {
                EngineSpec::Anthropic(a) => {
                    anthropic_plan(cfg, def, a, role, claude_argv, claude_model, base_env)
                }
                EngineSpec::Agy(a) => agy_plan(cfg, &def.id, a, role, claude_argv, base_env, timeout),
                EngineSpec::Dsh(d) => {
                    crate::dsh::plan(cfg, &def.id, d, role, claude_argv, base_env, cwd)
                }
            }
        }
    }
}

/// ENG-10 / D-36 / D-38 / D-42: one pure guard. `Some(message)` when the entry may not run on a
/// project of `project_profile` — an entry with `pin_profile` (which implies `allowed_profiles:
/// [pin_profile]`) or with `allowed_profiles` never borrows another profile's account or quota
/// (SPK-10). The run seam calls it before any run row exists; plan 06-22 binds it at the other
/// points where a team is chosen.
pub fn profile_refusal(
    def: &EngineDef,
    project_id: &str,
    project_profile: &str,
    role: &str,
) -> Option<String> {
    let allowed = def.effective_allowed_profiles()?;
    if allowed.iter().any(|p| p == project_profile) {
        return None;
    }
    Some(format!(
        "Engine '{}' chỉ dùng cho dự án profile {} — dự án '{project_id}' dùng profile {project_profile} (ranh giới work/personal, SPK-10). Đổi engine của vai trò {role} hoặc chọn đội hình khác.",
        def.id,
        allowed.join(", ")
    ))
}

/// An `anthropic` entry (ENG-01): the claude-family argv the role built, with argv[0] = the
/// entry's `command` and the value after `--model` = the entry's model for the role. Nothing
/// else in the argv changes (Pitfall A1): the guard hooks, the system prompt and the tool lists
/// stay exactly as the role built them.
fn anthropic_plan(
    cfg: &Config,
    def: &EngineDef,
    a: &AnthropicDef,
    role: &str,
    claude_argv: &[String],
    claude_model: &str,
    base_env: BTreeMap<String, String>,
) -> BrainResult<EnginePlan> {
    let model = match &a.base_url {
        Some(_) => a.models.get(role).or_else(|| a.models.get("dev")),
        None => a.models.get(role),
    }
    .cloned()
    .unwrap_or_else(|| claude_model.to_string());

    let mut argv = claude_argv.to_vec();
    if let Some(first) = argv.first_mut() {
        *first = a.command.clone();
    }
    match argv.iter().position(|x| x == "--model") {
        Some(i) if i + 1 < argv.len() => argv[i + 1] = model.clone(),
        _ => {
            argv.push("--model".to_string());
            argv.push(model.clone());
        }
    }

    let mut env = base_env;
    // A native entry (no `base_url`) runs on the project's profile with nothing injected.
    let native = a.base_url.is_none();
    let key = match (&a.base_url, a.key_env.as_deref()) {
        (Some(_), Some(key_env)) => Some(
            env.get(key_env)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    BrainError::error(format!(
                        "Chưa đặt biến môi trường {key_env} cho engine {} — đặt biến này trong shell chạy agent-brain (serve hoặc CLI) rồi chạy lại.",
                        def.id
                    ))
                })?,
        ),
        _ => None,
    };
    strip_key_envs(&mut env, cfg);

    // The endpoint variables go in AFTER `profile::build_env` already ran (Pitfall 5), so no
    // profile `unset_env` can drop them and nothing from the serve process env can shadow them.
    if let (Some(base_url), Some(key)) = (&a.base_url, &key) {
        env.retain(|k, _| !ENDPOINT_STRIPPED_ENV.contains(&k.as_str()));
        env.insert("ANTHROPIC_BASE_URL".to_string(), base_url.clone());
        env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), key.clone());
        env.insert(
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_string(),
            "1".to_string(),
        );
        for knob in &a.compat {
            match knob.as_str() {
                "disable_experimental_betas" => {
                    env.insert(
                        "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS".to_string(),
                        "1".to_string(),
                    );
                }
                "disable_adaptive_thinking" => {
                    env.insert(
                        "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING".to_string(),
                        "1".to_string(),
                    );
                }
                _ => {}
            }
        }
    }

    Ok(EnginePlan {
        engine: Engine::Named(def.id.clone()),
        db_engine: def.id.clone(),
        display: def.id.clone(),
        argv,
        env,
        model_label: model,
        stream: StreamFormat::Claude,
        stdout_file: false,
        redact: key.into_iter().collect(),
        response_file: None,
        native,
        unverified: !native && a.verified.is_none(),
        dsh: None,
    })
}

/// `--print-timeout` seconds for an agy run: the role timeout minus the margin when the timeout is
/// more than twice the margin, else half the timeout (never below 1). agy has no turn limit, so
/// time is the only bound (S7).
pub fn agy_print_timeout(timeout_secs: u64, margin: u64) -> u64 {
    if timeout_secs > margin.saturating_mul(2) {
        timeout_secs - margin
    } else {
        (timeout_secs / 2).max(1)
    }
}

/// The protocol file a read-only agy role answers with; agent-brain writes it from
/// `result.response` (D-05). Dev and qa have none.
pub fn response_file_for(role: &str) -> Option<&'static str> {
    match role {
        "lead" => Some(".plan.md"),
        "plan-review" => Some(".plan-review.md"),
        "code-review" => Some(".code-review.md"),
        _ => None,
    }
}

/// Extra argv appended for the Dev role only (D-05). Decided by the real agy 1.4.7 probe
/// (spikes/s7b/NOTES.md `## Quyết định`): the narrowest mode that can edit files. Lead and the
/// reviewers get none (they are read-only), and the skip flag stays unused.
pub const AGY_DEV_PERMISSION: &[&str] = &["--mode", "accept-edits"];

/// The `init.permission_mode` agy 1.4.7 reports for a run of `role`. The probe showed
/// `--mode accept-edits` is NOT reflected in `init` (it stays `request-review`), so every role
/// expects `request-review`; the real proof for Dev is the diff (D-05).
pub fn agy_expected_permission_mode(role: &str) -> &'static str {
    let _ = role;
    "request-review"
}

fn agy_plan(
    cfg: &Config,
    id: &str,
    acfg: &AgyDef,
    role: &str,
    claude_argv: &[String],
    base_env: BTreeMap<String, String>,
    timeout: Duration,
) -> BrainResult<EnginePlan> {
    let (input, prompt) = claude_argv_parts(claude_argv)
        .ok_or_else(|| BrainError::error("Không dựng được lệnh agy từ lệnh claude"))?;
    let response_file = response_file_for(role);
    let text = match response_file {
        Some(file) if role != "dev" => format!(
            "{prompt}\n\n---\nBạn đang chạy ở chế độ chỉ đọc: không sửa và không tạo file nào. Bạn không chạy được lệnh shell (bị từ chối quyền); dùng ngữ cảnh đã cho và công cụ đọc file. Nếu thiếu thông tin, vẫn phải trả file đầy đủ, không được trả rỗng. Trả lời bằng TOÀN BỘ nội dung file {file} theo đúng định dạng ở trên, không thêm lời dẫn, không bọc trong khối mã.\n\n---\n{input}"
        ),
        _ => format!("{prompt}\n\n---\n{input}"),
    };

    let secs = agy_print_timeout(timeout.as_secs(), acfg.print_timeout_margin_secs);
    let mut argv = vec![
        acfg.command.clone(),
        "-p".to_string(),
        text,
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--print-timeout".to_string(),
        format!("{secs}s"),
    ];
    if let Some(m) = &acfg.model {
        argv.push("--model".to_string());
        argv.push(m.clone());
    }
    if role == "dev" {
        argv.push("--mode".to_string());
        argv.push(acfg.dev_permission.clone());
    }

    // agy never sees claude credentials or config, nor any engine's key (T-06-35, T-06-45).
    let mut env = base_env;
    env.retain(|k, _| {
        !(k.starts_with("ANTHROPIC_")
            || k == "CLAUDE_CONFIG_DIR"
            || k == "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"
            || k == "CLAUDE_CODE_SUBAGENT_MODEL")
    });
    strip_key_envs(&mut env, cfg);

    Ok(EnginePlan {
        engine: Engine::Named(id.to_string()),
        db_engine: id.to_string(),
        display: id.to_string(),
        argv,
        env,
        model_label: acfg.model.clone().unwrap_or_else(|| "mặc định".to_string()),
        stream: StreamFormat::Agy,
        stdout_file: true,
        redact: Vec::new(),
        response_file,
        native: false,
        unverified: false,
        dsh: None,
    })
}

/// `(-p input, --append-system-prompt text)` of a claude-family argv; `None` when either is missing.
pub fn claude_argv_parts(argv: &[String]) -> Option<(String, String)> {
    let after = |flag: &str| {
        argv.iter()
            .position(|a| a == flag)
            .and_then(|i| argv.get(i + 1))
            .cloned()
    };
    Some((after("-p")?, after("--append-system-prompt")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_argv_parts_reads_input_and_prompt() {
        let cfg = test_cfg(serde_json::json!({}));
        let spec = crate::roles::RoleSpec {
            role: crate::roles::Role::Dev,
            user_input: "làm theo plan".to_string(),
            allowed_tools: vec![],
            disallowed_tools: vec![],
            extra_args: vec![],
        };
        let argv = crate::roles::build_command_with_prompt(&spec, &cfg, "luật của dev").unwrap();
        assert_eq!(
            claude_argv_parts(&argv),
            Some(("làm theo plan".to_string(), "luật của dev".to_string()))
        );
        assert_eq!(claude_argv_parts(&["claude".to_string()]), None);
        assert_eq!(
            claude_argv_parts(&["-p".to_string(), "x".to_string()]),
            None
        );
    }

    /// `plan_for` with a fixed working directory (only a dsh run reads it).
    fn run_plan(
        cfg: &Config,
        resolved: &ResolvedEngine,
        role: &str,
        argv: &[String],
        model: &str,
        env: BTreeMap<String, String>,
        timeout: Duration,
    ) -> BrainResult<EnginePlan> {
        plan_for(
            cfg,
            resolved,
            role,
            argv,
            model,
            env,
            timeout,
            Path::new("/"),
        )
    }

    fn named(id: &str) -> ResolvedEngine {
        ResolvedEngine {
            engine: Engine::Named(id.to_string()),
            team: Some("doi-x".to_string()),
            wait: None,
        }
    }

    fn claude_argv_of(model: &str) -> Vec<String> {
        [
            "claude",
            "-p",
            "việc",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            model,
            "--append-system-prompt",
            "luật",
            "--settings",
            "/run/settings.json",
            "--max-turns",
            "7",
            "--allowedTools",
            "Read",
            "--disallowedTools",
            "WebFetch",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn env_of(names: &[&str]) -> BTreeMap<String, String> {
        names
            .iter()
            .map(|k| (k.to_string(), format!("value-of-{k}")))
            .collect()
    }

    #[test]
    fn plan_for_claude_keeps_argv_and_env() {
        let cfg = test_cfg(serde_json::json!({}));
        let resolved = ResolvedEngine {
            engine: Engine::Claude,
            team: Some("mac-dinh".to_string()),
            wait: None,
        };
        let argv = vec!["claude".to_string(), "-p".to_string(), "x".to_string()];
        let mut env = BTreeMap::new();
        env.insert("A".to_string(), "1".to_string());
        let p = run_plan(
            &cfg,
            &resolved,
            "dev",
            &argv,
            "sonnet",
            env.clone(),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(p.argv, argv);
        assert_eq!(p.env, env);
        assert_eq!(p.db_engine, "claude");
        assert_eq!(p.display, "claude");
        assert_eq!(p.model_label, "sonnet");
        assert_eq!(p.stream, StreamFormat::Claude);
        assert!(p.native);
        assert!(!p.unverified);
        assert_eq!(p.dsh, None);

        // A person never runs a process.
        let r = ResolvedEngine {
            engine: Engine::Human("ha".to_string()),
            team: None,
            wait: None,
        };
        assert!(run_plan(&cfg, &r, "dev", &argv, "m", BTreeMap::new(), Duration::ZERO).is_err());
    }

    #[test]
    fn claude_env_loses_every_key_variable() {
        // Pitfall A2: claude never sees the key of any configured engine.
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "gw", "kind": "anthropic", "base_url": "https://gw.invalid/x", "key_env": "GW_KEY",
             "models": {"dev": "m"}},
            {"id": "ds", "kind": "anthropic", "base_url": "https://ds.invalid/x", "key_env": "DS_KEY",
             "models": {"dev": "m"}},
        ]}));
        let resolved = ResolvedEngine {
            engine: Engine::Claude,
            team: None,
            wait: None,
        };
        let p = run_plan(
            &cfg,
            &resolved,
            "dev",
            &claude_argv_of("sonnet"),
            "sonnet",
            env_of(&["GW_KEY", "DS_KEY", "HOME", "PATH"]),
            Duration::from_secs(5),
        )
        .unwrap();
        let names: Vec<&str> = p.env.keys().map(String::as_str).collect();
        assert_eq!(names, ["HOME", "PATH"]);
    }

    #[test]
    fn anthropic_plan_builds_argv_and_env() {
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "gw", "kind": "anthropic", "command": "gw-claude", "base_url": "https://gw.invalid/x",
             "key_env": "GW_KEY", "compat": ["disable_experimental_betas", "disable_adaptive_thinking"],
             "models": {"lead": "m-lead", "dev": "m-dev"}},
            {"id": "other", "kind": "anthropic", "base_url": "https://o.invalid/x", "key_env": "OTHER_KEY",
             "models": {"dev": "m-o"}, "verified": "2026-10-28 E-09"},
        ]}));
        let mut env = env_of(&[
            "GW_KEY",
            "OTHER_KEY",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "CLAUDE_CODE_SUBAGENT_MODEL",
            "CLAUDE_CONFIG_DIR",
            "HOME",
            "PATH",
        ]);
        env.insert(
            "GW_KEY".to_string(),
            "test-planted-gw-key-0123456789".to_string(),
        );
        let argv = claude_argv_of("opus");

        // dev: the role's model; lead: its own entry; qa: falls back to models.dev.
        for (role, model) in [("dev", "m-dev"), ("lead", "m-lead"), ("qa", "m-dev")] {
            let p = run_plan(
                &cfg,
                &named("gw"),
                role,
                &argv,
                "opus",
                env.clone(),
                Duration::from_secs(5),
            )
            .unwrap();
            // Only argv[0] and the value after --model differ from the role's own argv.
            assert_eq!(p.argv[0], "gw-claude");
            let m = p.argv.iter().position(|a| a == "--model").unwrap();
            assert_eq!(p.argv[m + 1], model, "{role}");
            let mut expected = argv.clone();
            expected[0] = "gw-claude".to_string();
            expected[m + 1] = model.to_string();
            assert_eq!(p.argv, expected, "{role}");
            assert_eq!(p.model_label, model);
            assert_eq!(p.db_engine, "gw");
            assert_eq!(p.display, "gw");
            assert_eq!(p.stream, StreamFormat::Claude);
            assert!(!p.stdout_file);
            assert!(!p.native);
            assert!(p.unverified, "no `verified` text: marked (D-40)");
            assert_eq!(p.redact, ["test-planted-gw-key-0123456789"]);

            assert_eq!(p.env["ANTHROPIC_BASE_URL"], "https://gw.invalid/x");
            assert_eq!(
                p.env["ANTHROPIC_AUTH_TOKEN"],
                "test-planted-gw-key-0123456789"
            );
            assert_eq!(p.env["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"], "1");
            assert_eq!(p.env["CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS"], "1");
            assert_eq!(p.env["CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING"], "1");
            for gone in [
                "GW_KEY",
                "OTHER_KEY",
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "CLAUDE_CODE_SUBAGENT_MODEL",
            ] {
                assert!(!p.env.contains_key(gone), "{gone} reached the child");
            }
            // The profile env (D-07) is otherwise untouched.
            assert_eq!(p.env["CLAUDE_CONFIG_DIR"], "value-of-CLAUDE_CONFIG_DIR");
            assert_eq!(p.env["HOME"], "value-of-HOME");
        }

        // A verified endpoint carries no mark, and no compat knob unless asked for.
        let p = run_plan(
            &cfg,
            &named("other"),
            "dev",
            &argv,
            "opus",
            env.clone(),
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(!p.unverified);
        assert!(!p.env.contains_key("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS"));
        assert!(!p.env.contains_key("CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING"));
        assert_eq!(p.env["ANTHROPIC_BASE_URL"], "https://o.invalid/x");
        // `other` has no key of its own in this env map: refused with the D-03 text.
        env.remove("OTHER_KEY");
        let e = run_plan(
            &cfg,
            &named("other"),
            "dev",
            &argv,
            "opus",
            env.clone(),
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "Chưa đặt biến môi trường OTHER_KEY cho engine other — đặt biến này trong shell chạy agent-brain (serve hoặc CLI) rồi chạy lại."
        );
        // A blank value is a missing one.
        env.insert("OTHER_KEY".to_string(), "   ".to_string());
        assert!(run_plan(
            &cfg,
            &named("other"),
            "dev",
            &argv,
            "opus",
            env,
            Duration::from_secs(5)
        )
        .is_err());
    }

    #[test]
    fn native_anthropic_entry_injects_nothing() {
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "claude-max", "kind": "anthropic", "command": "max-claude", "pin_profile": "personal",
             "models": {"lead": "opus"}},
            {"id": "gw", "kind": "anthropic", "base_url": "https://gw.invalid/x", "key_env": "GW_KEY",
             "models": {"dev": "m"}},
        ]}));
        let argv = claude_argv_of("sonnet");
        let env = env_of(&["GW_KEY", "ANTHROPIC_MODEL", "CLAUDE_CONFIG_DIR", "PATH"]);
        let p = run_plan(
            &cfg,
            &named("claude-max"),
            "lead",
            &argv,
            "sonnet",
            env,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(p.native);
        assert!(!p.unverified);
        assert!(p.redact.is_empty());
        assert_eq!(p.argv[0], "max-claude");
        let m = p.argv.iter().position(|a| a == "--model").unwrap();
        assert_eq!(p.argv[m + 1], "opus", "models.lead of the entry");
        assert_eq!(p.model_label, "opus");
        // No key variable, nothing injected, the profile's own env kept as it was.
        let names: Vec<&str> = p.env.keys().map(String::as_str).collect();
        assert_eq!(names, ["ANTHROPIC_MODEL", "CLAUDE_CONFIG_DIR", "PATH"]);

        // A role the entry has no model for keeps the model the role already chose.
        let p = run_plan(
            &cfg,
            &named("claude-max"),
            "dev",
            &argv,
            "sonnet",
            BTreeMap::new(),
            Duration::from_secs(5),
        )
        .unwrap();
        let m = p.argv.iter().position(|a| a == "--model").unwrap();
        assert_eq!(p.argv[m + 1], "sonnet");
        assert_eq!(p.model_label, "sonnet");
    }

    #[test]
    fn unknown_id_and_dsh_non_dev_are_refused() {
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "dsh", "kind": "dsh", "container": "dsh-x"},
        ]}));
        let argv = claude_argv_of("sonnet");
        let e = run_plan(
            &cfg,
            &named("khong-co"),
            "dev",
            &argv,
            "m",
            BTreeMap::new(),
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "Engine 'khong-co' của đội hình 'doi-x' không có trong config.json (engines) — thêm engine hoặc đổi đội hình rồi chạy lại."
        );
        // A run with no team reads `-` in the team slot.
        let r = ResolvedEngine {
            engine: Engine::Named("khong-co".to_string()),
            team: None,
            wait: None,
        };
        let e = run_plan(&cfg, &r, "dev", &argv, "m", BTreeMap::new(), Duration::ZERO).unwrap_err();
        assert!(e.message.contains("đội hình '-'"), "{}", e.message);

        // The default `deepseek` / `agy` ids do not exist in this user list: never mapped elsewhere.
        for id in ["deepseek", "agy"] {
            assert!(run_plan(
                &cfg,
                &named(id),
                "dev",
                &argv,
                "m",
                BTreeMap::new(),
                Duration::ZERO
            )
            .is_err());
        }

        // dsh is dispatched to `dsh::plan` (plan 06-23): a read-only role is refused there,
        // before any docker call (the run seam of the dev role is covered by engine_dsh.rs).
        let e = run_plan(
            &cfg,
            &named("dsh"),
            "lead",
            &argv,
            "m",
            BTreeMap::new(),
            Duration::ZERO,
        )
        .unwrap_err();
        assert_eq!(
            e.message,
            "Engine 'dsh' là kiểu dsh, chỉ dùng cho vai trò dev."
        );
    }

    #[test]
    fn profile_refusal_rules() {
        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "claude-max", "kind": "anthropic", "pin_profile": "personal"},
            {"id": "dsh", "kind": "dsh", "container": "dsh-x", "allowed_profiles": ["personal"]},
            {"id": "both", "kind": "agy", "allowed_profiles": ["work", "personal"]},
            {"id": "free", "kind": "agy"},
        ]}));
        let def = |id: &str| cfg.engines.get(id).unwrap();

        // pin_profile personal: refused on a work project with the exact D-36 text.
        assert_eq!(
            profile_refusal(def("claude-max"), "web-ban-hang", "work", "dev").as_deref(),
            Some("Engine 'claude-max' chỉ dùng cho dự án profile personal — dự án 'web-ban-hang' dùng profile work (ranh giới work/personal, SPK-10). Đổi engine của vai trò dev hoặc chọn đội hình khác.")
        );
        // ... and allowed on the personal one, where it runs on that project's own profile.
        assert_eq!(
            profile_refusal(def("claude-max"), "web-ban-hang", "personal", "dev"),
            None
        );
        // allowed_profiles works for any kind.
        let m = profile_refusal(def("dsh"), "p", "work", "dev").expect("dsh is personal-only");
        assert!(m.starts_with(
            "Engine 'dsh' chỉ dùng cho dự án profile personal — dự án 'p' dùng profile work"
        ));
        assert_eq!(profile_refusal(def("dsh"), "p", "personal", "dev"), None);
        // A list of two is allowed on both and names both in a refusal.
        for profile in ["work", "personal"] {
            assert_eq!(profile_refusal(def("both"), "p", profile, "dev"), None);
        }
        let m = profile_refusal(def("both"), "p", "other", "qa").unwrap();
        assert!(m.contains("profile work, personal"), "{m}");
        assert!(
            m.ends_with("Đổi engine của vai trò qa hoặc chọn đội hình khác."),
            "{m}"
        );
        // No restriction: never refused.
        for profile in ["work", "personal"] {
            assert_eq!(profile_refusal(def("free"), "p", profile, "dev"), None);
        }
    }

    #[test]
    fn agy_print_timeout_rules() {
        assert_eq!(agy_print_timeout(1800, 30), 1770);
        assert_eq!(agy_print_timeout(61, 30), 31);
        assert_eq!(agy_print_timeout(60, 30), 30);
        assert_eq!(agy_print_timeout(30, 30), 15);
        assert_eq!(agy_print_timeout(1, 30), 1);
        assert_eq!(agy_print_timeout(0, 30), 1);
    }

    #[test]
    fn response_files_by_role() {
        assert_eq!(response_file_for("lead"), Some(".plan.md"));
        assert_eq!(response_file_for("plan-review"), Some(".plan-review.md"));
        assert_eq!(response_file_for("code-review"), Some(".code-review.md"));
        assert_eq!(response_file_for("dev"), None);
        assert_eq!(response_file_for("qa"), None);
    }

    #[test]
    fn plan_for_agy_builds_headless_argv_and_strips_env() {
        let cfg = test_cfg(serde_json::json!({
            "engines": {"agy": {"command": "agy-bin", "model": "gemini-x"}}
        }));
        let resolved = ResolvedEngine {
            engine: Engine::Named("agy".to_string()),
            team: None,
            wait: None,
        };
        let claude = vec![
            "claude".to_string(),
            "-p".to_string(),
            "yêu cầu".to_string(),
            "--append-system-prompt".to_string(),
            "luật".to_string(),
        ];
        let mut env = BTreeMap::new();
        for k in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CONFIG_DIR",
            "DEEPSEEK_API_KEY",
            "HOME",
            "PATH",
        ] {
            env.insert(k.to_string(), "v".to_string());
        }
        let p = run_plan(
            &cfg,
            &resolved,
            "lead",
            &claude,
            "opus",
            env,
            Duration::from_secs(1800),
        )
        .unwrap();
        assert_eq!(p.argv[0], "agy-bin");
        assert_eq!(p.argv[1], "-p");
        assert!(p.argv[2].starts_with("luật\n\n---\n"));
        assert!(p.argv[2].contains(".plan.md"));
        assert!(p.argv[2].ends_with("yêu cầu"));
        assert_eq!(
            &p.argv[3..7],
            ["--output-format", "stream-json", "--print-timeout", "1770s"]
        );
        assert_eq!(&p.argv[7..], ["--model", "gemini-x"]);
        assert_eq!(p.model_label, "gemini-x");
        assert_eq!(p.db_engine, "agy");
        assert_eq!(p.stream, StreamFormat::Agy);
        assert!(p.stdout_file);
        assert_eq!(p.response_file, Some(".plan.md"));
        let names: Vec<&str> = p.env.keys().map(String::as_str).collect();
        assert_eq!(names, ["HOME", "PATH"]);

        let bad = run_plan(
            &cfg,
            &resolved,
            "lead",
            &["claude".to_string()],
            "m",
            BTreeMap::new(),
            Duration::from_secs(60),
        )
        .unwrap_err();
        assert_eq!(bad.message, "Không dựng được lệnh agy từ lệnh claude");
    }

    #[test]
    fn agy_permission_only_for_dev() {
        let cfg = test_cfg(serde_json::json!({
            "engines": {"agy": {"model": "gemini-x"}}
        }));
        let resolved = ResolvedEngine {
            engine: Engine::Named("agy".to_string()),
            team: None,
            wait: None,
        };
        let claude = vec![
            "claude".to_string(),
            "-p".to_string(),
            "việc".to_string(),
            "--append-system-prompt".to_string(),
            "luật".to_string(),
        ];
        for role in ["lead", "plan-review", "code-review", "qa"] {
            let p = run_plan(
                &cfg,
                &resolved,
                role,
                &claude,
                "m",
                BTreeMap::new(),
                Duration::from_secs(600),
            )
            .unwrap();
            for bad in ["--mode", "--sandbox", "--dangerously-skip-permissions"] {
                assert!(!p.argv.iter().any(|a| a == bad), "{role} got {bad}");
            }
        }
        let p = run_plan(
            &cfg,
            &resolved,
            "dev",
            &claude,
            "m",
            BTreeMap::new(),
            Duration::from_secs(600),
        )
        .unwrap();
        assert!(p.argv.ends_with(
            &AGY_DEV_PERMISSION
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        ));
        assert_eq!(AGY_DEV_PERMISSION, ["--mode", "accept-edits"]);
        assert_eq!(p.response_file, None);
        // The prompt for Dev is not the read-only variant.
        assert!(!p.argv[2].contains("chế độ chỉ đọc"));
        assert_eq!(agy_expected_permission_mode("dev"), "request-review");
        assert_eq!(agy_expected_permission_mode("lead"), "request-review");
    }

    fn test_cfg(extra: serde_json::Value) -> Config {
        let mut v: serde_json::Value =
            serde_json::from_str(crate::config::DEFAULT_CONFIG_JSON).unwrap();
        if let Some(o) = extra.as_object() {
            for (k, val) in o {
                v.as_object_mut().unwrap().insert(k.clone(), val.clone());
            }
        }
        crate::config::config_from_value(&v).unwrap()
    }

    fn make_exec(dir: &Path, name: &str) {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn readiness_states() {
        let dir = tempfile::tempdir().unwrap();
        make_exec(dir.path(), "claude");
        make_exec(dir.path(), "agy");
        // A non-executable file named like a command does not count.
        std::fs::write(dir.path().join("plain"), "x").unwrap();

        let path = dir.path().to_str().unwrap().to_string();
        let planted = "planted-value-1234567890";
        let mut env = BTreeMap::new();
        env.insert("PATH".to_string(), path.clone());
        env.insert("DEEPSEEK_API_KEY".to_string(), planted.to_string());

        let cfg = test_cfg(serde_json::json!({}));
        let all = readiness(&cfg, &env, 3);
        assert_eq!(
            all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["claude", "deepseek", "agy", "người"]
        );
        for s in all.iter().filter(|s| s.name != "deepseek") {
            assert_eq!(s.readiness, "san_sang", "{}", s.name);
            assert_eq!(s.word, "sẵn sàng");
            assert_eq!(s.reason, None);
        }
        // D-40: the default deepseek entry has no `verified`, so a set key reads `chưa kiểm`.
        assert_eq!(all[1].readiness, "chua_kiem");
        assert_eq!(all[1].word, "chưa kiểm");
        assert_eq!(all[1].reason.as_deref(), Some(REASON_UNVERIFIED));
        assert_eq!(all[0].command.as_deref(), Some("claude"));
        assert_eq!(all[1].key_env.as_deref(), Some("DEEPSEEK_API_KEY"));
        assert_eq!(all[1].model.as_deref(), Some("opus, sonnet"));
        assert_eq!(all[2].command.as_deref(), Some("agy"));
        assert_eq!(all[3].detail.as_deref(), Some("3 nhân sự"));
        assert_eq!(
            all[1].description,
            "claude chạy trên máy với endpoint của DeepSeek."
        );

        // The planted key value never leaves the environment map.
        let json = serde_json::to_string(&all).unwrap();
        assert!(!json.contains(planted), "key value leaked: {json}");
        assert!(json.contains("DEEPSEEK_API_KEY"));

        // No key (or a blank one): deepseek thieu_key with the exact reason.
        for blank in [None, Some("   ")] {
            let mut env2 = env.clone();
            env2.remove("DEEPSEEK_API_KEY");
            if let Some(b) = blank {
                env2.insert("DEEPSEEK_API_KEY".to_string(), b.to_string());
            }
            let r = readiness(&cfg, &env2, 0);
            assert_eq!(r[1].readiness, "thieu_key");
            assert_eq!(r[1].word, "thiếu key");
            assert_eq!(
                r[1].reason.as_deref(),
                Some("Chưa đặt biến môi trường DEEPSEEK_API_KEY.")
            );
            assert_eq!(r[0].readiness, "san_sang");
            assert_eq!(r[3].detail.as_deref(), Some("0 nhân sự"));
        }

        // Missing agy command.
        let cfg2 = test_cfg(serde_json::json!({"engines": {"agy": {"command": "khong-co-lenh"}}}));
        let r = readiness(&cfg2, &env, 1);
        assert_eq!(r[2].readiness, "thieu_lenh");
        assert_eq!(r[2].word, "thiếu lệnh");
        assert_eq!(
            r[2].reason.as_deref(),
            Some("Không tìm thấy lệnh khong-co-lenh trên máy.")
        );
        assert_eq!(r[1].readiness, "chua_kiem");

        // An absolute command path must be an executable file.
        let abs = dir.path().join("agy");
        let cfg3 =
            test_cfg(serde_json::json!({"engines": {"agy": {"command": abs.to_str().unwrap()}}}));
        assert_eq!(
            readiness(&cfg3, &BTreeMap::new(), 1)[2].readiness,
            "san_sang"
        );
        let cfg4 = test_cfg(
            serde_json::json!({"engines": {"agy": {"command": dir.path().join("plain").to_str().unwrap()}}}),
        );
        assert_eq!(readiness(&cfg4, &env, 1)[2].readiness, "thieu_lenh");

        // No PATH at all: bare commands are not found.
        let none = readiness(&cfg, &BTreeMap::new(), 0);
        assert_eq!(none[0].readiness, "thieu_lenh");
        assert_eq!(none[3].readiness, "san_sang");

        // Missing command wins over a missing key (the command is the first thing to fix).
        let mut env_nokey = BTreeMap::new();
        env_nokey.insert("PATH".to_string(), "/khong/co/thu-muc".to_string());
        assert_eq!(readiness(&cfg, &env_nokey, 0)[1].readiness, "thieu_lenh");
    }

    fn gateway_cfg(verified: Option<&str>) -> Config {
        let mut entry = serde_json::json!({
            "id": "gw", "kind": "anthropic", "command": "claude",
            "base_url": "https://gw.example.test/x", "key_env": "GW_KEY",
            "models": {"dev": "m-dev"}
        });
        if let Some(v) = verified {
            entry["verified"] = serde_json::json!(v);
        }
        test_cfg(serde_json::json!({ "engines": [entry] }))
    }

    fn no_probe(_: &str, _: &str) -> Option<String> {
        panic!("the container probe must not run");
    }

    #[test]
    fn readiness_chua_kiem() {
        let dir = tempfile::tempdir().unwrap();
        make_exec(dir.path(), "claude");
        let mut env = BTreeMap::new();
        env.insert("PATH".to_string(), dir.path().to_str().unwrap().to_string());
        env.insert("GW_KEY".to_string(), "planted-gw-key-zzz".to_string());

        // No `verified`: runs, but is marked.
        let r = readiness_with(&gateway_cfg(None), &env, 0, &no_probe);
        assert_eq!(r[1].readiness, "chua_kiem");
        assert_eq!(r[1].word, "chưa kiểm");
        assert_eq!(
            r[1].reason.as_deref(),
            Some("Endpoint chưa có bằng chứng E-09 — chạy spikes/e09/run.sh rồi ghi \"verified\" cho engine này trong config.json.")
        );
        assert_eq!(r[1].evidence, None);

        // With `verified`: san_sang and the text is carried.
        let r = readiness_with(&gateway_cfg(Some("2026-10-28 E-09")), &env, 0, &no_probe);
        assert_eq!(r[1].readiness, "san_sang");
        assert_eq!(r[1].reason, None);
        assert_eq!(r[1].evidence.as_deref(), Some("2026-10-28 E-09"));

        // Key unset (or blank): thieu_key wins over chua_kiem.
        let mut no_key = env.clone();
        no_key.remove("GW_KEY");
        let r = readiness_with(&gateway_cfg(None), &no_key, 0, &no_probe);
        assert_eq!(r[1].readiness, "thieu_key");
        assert_eq!(
            r[1].reason.as_deref(),
            Some("Chưa đặt biến môi trường GW_KEY.")
        );
        no_key.insert("GW_KEY".to_string(), "  ".to_string());
        let r = readiness_with(&gateway_cfg(None), &no_key, 0, &no_probe);
        assert_eq!(r[1].readiness, "thieu_key");

        // Command missing: thieu_lenh wins over both.
        let mut no_cmd = env.clone();
        no_cmd.insert("PATH".to_string(), "/khong/co/thu-muc".to_string());
        let r = readiness_with(&gateway_cfg(None), &no_cmd, 0, &no_probe);
        assert_eq!(r[1].readiness, "thieu_lenh");
        assert_eq!(
            r[1].reason.as_deref(),
            Some("Không tìm thấy lệnh claude trên máy.")
        );

        // The planted key value never reaches a row.
        let json =
            serde_json::to_string(&readiness_with(&gateway_cfg(None), &env, 0, &no_probe)).unwrap();
        assert!(!json.contains("planted-gw-key-zzz"), "{json}");
        assert!(!json.contains("gw.example.test"), "{json}");

        // A native entry has no endpoint to verify: san_sang by its command alone.
        let native = test_cfg(serde_json::json!({"engines": [
            {"id": "claude-max", "kind": "anthropic", "command": "claude",
             "pin_profile": "personal", "models": {"dev": "opus"}}
        ]}));
        let r = readiness_with(&native, &env, 0, &no_probe);
        assert_eq!(r[1].readiness, "san_sang");
        assert_eq!(r[1].pin_profile.as_deref(), Some("personal"));
        assert_eq!(
            r[1].allowed_profiles.as_deref(),
            Some(&["personal".to_string()][..])
        );
        let r = readiness_with(&native, &no_cmd, 0, &no_probe);
        assert_eq!(r[1].readiness, "thieu_lenh");
    }

    #[test]
    fn readiness_dsh() {
        let dir = tempfile::tempdir().unwrap();
        make_exec(dir.path(), "claude");
        make_exec(dir.path(), "docker-fake");
        let docker = dir.path().join("docker-fake");
        let mut env = BTreeMap::new();
        env.insert("PATH".to_string(), dir.path().to_str().unwrap().to_string());
        let dsh = |docker: &str| {
            test_cfg(serde_json::json!({"engines": [
                {"id": "dsh-x", "kind": "dsh", "label": "Hộp đen", "docker": docker,
                 "container": "dsh-x", "dsh_profile": "headless",
                 "allowed_profiles": ["personal"]}
            ]}))
        };
        let cfg = dsh(docker.to_str().unwrap());

        let up = |_: &str, c: &str| {
            assert_eq!(c, "dsh-x");
            Some("Up 3 minutes".to_string())
        };
        let r = readiness_with(&cfg, &env, 0, &up);
        assert_eq!(r[1].name, "dsh-x");
        assert_eq!(r[1].kind, "dsh");
        assert_eq!(r[1].readiness, "san_sang");
        assert_eq!(r[1].reason, None);
        assert_eq!(r[1].usage, "none");
        assert_eq!(r[1].container.as_deref(), Some("dsh-x"));
        assert_eq!(r[1].label.as_deref(), Some("Hộp đen"));
        assert_eq!(r[1].model.as_deref(), Some("headless"));
        assert_eq!(
            r[1].allowed_profiles.as_deref(),
            Some(&["personal".to_string()][..])
        );

        for status in [Some("Exited (0) 2 hours ago".to_string()), None] {
            let probe = move |_: &str, _: &str| status.clone();
            let r = readiness_with(&cfg, &env, 0, &probe);
            assert_eq!(r[1].readiness, "thieu_lenh");
            assert_eq!(r[1].word, "thiếu lệnh");
            assert_eq!(r[1].reason.as_deref(), Some("Container dsh-x chưa chạy."));
        }

        // A docker that is not an executable: no probe, the usual missing-command reason.
        let cfg = dsh("/khong/co/docker-test");
        let r = readiness_with(&cfg, &env, 0, &no_probe);
        assert_eq!(r[1].readiness, "thieu_lenh");
        assert_eq!(
            r[1].reason.as_deref(),
            Some("Không tìm thấy lệnh /khong/co/docker-test trên máy.")
        );

        // `người` and the default list carry the no-token hint where it applies.
        let last = r.last().unwrap();
        assert_eq!(last.name, "người");
        assert_eq!(last.usage, "none");
        assert_eq!(last.kind, "người");
        assert_eq!(r[0].usage, "tokens");
    }

    #[test]
    fn docker_status_probe_reads_first_line_and_fails_soft() {
        let dir = tempfile::tempdir().unwrap();
        let script = |name: &str, body: &str| {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.path().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p.to_str().unwrap().to_string()
        };
        // The argv is exactly `ps --filter name=^<container>$ --format {{.Status}}`.
        let ok = script(
            "ok",
            "[ \"$1 $2 $3 $4 $5\" = 'ps --filter name=^c1$ --format {{.Status}}' ] && echo 'Up 5 seconds' && echo second",
        );
        assert_eq!(
            docker_status_probe(&ok, "c1").as_deref(),
            Some("Up 5 seconds")
        );
        let fail = script("fail", "echo nope; exit 1");
        assert_eq!(docker_status_probe(&fail, "c1"), None);
        let empty = script("empty", "exit 0");
        assert_eq!(docker_status_probe(&empty, "c1"), None);
        assert_eq!(docker_status_probe("/khong/co/docker", "c1"), None);
    }

    #[test]
    fn readiness_list_order() {
        let cfg = test_cfg(serde_json::json!({}));
        let r = readiness(&cfg, &BTreeMap::new(), 0);
        assert_eq!(
            r.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["claude", "deepseek", "agy", "người"]
        );
        assert_eq!(
            r.iter().map(|s| s.kind.as_str()).collect::<Vec<_>>(),
            ["claude", "anthropic", "agy", "người"]
        );

        let cfg = test_cfg(serde_json::json!({"engines": [
            {"id": "zz", "kind": "agy", "command": "agy"},
            {"id": "aa", "kind": "anthropic", "command": "claude",
             "base_url": "https://a.example.test", "key_env": "AA_KEY",
             "models": {"dev": "m"}},
            {"id": "dd", "kind": "dsh", "docker": "docker", "container": "c",
             "dsh_profile": "headless"}
        ]}));
        let probe = |_: &str, _: &str| None;
        let r = readiness_with(&cfg, &BTreeMap::new(), 2, &probe);
        assert_eq!(
            r.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["claude", "zz", "aa", "dd", "người"]
        );
        assert_eq!(
            r.iter().map(|s| s.kind.as_str()).collect::<Vec<_>>(),
            ["claude", "agy", "anthropic", "dsh", "người"]
        );
        assert_eq!(
            r.iter().map(|s| s.usage).collect::<Vec<_>>(),
            ["tokens", "tokens", "tokens", "none", "none"]
        );
    }

    #[test]
    fn find_command_rules() {
        let dir = tempfile::tempdir().unwrap();
        make_exec(dir.path(), "tool");
        let p = dir.path().to_str().unwrap();
        assert!(find_command("tool", Some(p)));
        assert!(find_command("tool", Some(&format!("/khong/co:{p}"))));
        assert!(!find_command("tool", Some("")));
        assert!(!find_command("tool", None));
        assert!(!find_command("", Some(p)));
        assert!(!find_command("khac", Some(p)));
        assert!(find_command(
            dir.path().join("tool").to_str().unwrap(),
            None
        ));
    }

    #[test]
    fn engine_parse() {
        assert_eq!(Engine::parse("claude"), Some(Engine::Claude));
        assert_eq!(
            Engine::parse("deepseek"),
            Some(Engine::Named("deepseek".to_string()))
        );
        assert_eq!(
            Engine::parse(" agy "),
            Some(Engine::Named("agy".to_string()))
        );
        assert_eq!(
            Engine::parse("vcadm-deepseek"),
            Some(Engine::Named("vcadm-deepseek".to_string()))
        );
        assert_eq!(
            Engine::parse("người:ha"),
            Some(Engine::Human("ha".to_string()))
        );

        // The NFD spelling parses to the same value.
        let nfd: String = "người:ha".nfd().collect();
        assert_ne!(nfd, "người:ha", "test needs a genuinely different spelling");
        assert_eq!(Engine::parse(&nfd), Some(Engine::Human("ha".to_string())));

        for bad in [
            "Claude",
            "người:",
            "người:Ha",
            "nguoi:ha",
            "bad id",
            "GPT",
            "",
            "người",
        ] {
            assert_eq!(Engine::parse(bad), None, "{bad:?} must not parse");
        }
        // An id is at most 32 bytes.
        let long = "a".repeat(33);
        assert_eq!(Engine::parse(&long), None);
        assert_eq!(
            Engine::parse(&"a".repeat(32)),
            Some(Engine::Named("a".repeat(32)))
        );

        let h = Engine::Human("ha".to_string());
        assert_eq!(h.as_db_str(), "người");
        assert_eq!(h.to_config_string(), "người:ha");
        assert!(h.is_human());
        assert!(!Engine::Claude.is_human());
        assert_eq!(
            Engine::Named("deepseek".to_string()).to_config_string(),
            "deepseek"
        );

        let nfc: String = ENGINE_HUMAN.nfc().collect();
        assert_eq!(nfc, ENGINE_HUMAN, "ENGINE_HUMAN must be NFC");
        assert_eq!(
            engine_order(&test_cfg(serde_json::json!({}))),
            ["claude", "deepseek", "agy", "người"]
        );
    }

    #[test]
    fn readiness_keys_and_words() {
        assert_eq!(Readiness::SanSang.key(), "san_sang");
        assert_eq!(Readiness::ThieuKey.word(), "thiếu key");
        assert_eq!(Readiness::ThieuLenh.key(), "thieu_lenh");
        assert_eq!(Readiness::ChuaKiem.word(), "chưa kiểm");
    }

    #[test]
    fn agy_readonly_prompt_guidance() {
        let cfg = test_cfg(serde_json::json!({
            "engines": {"agy": {"model": "gemini-x"}}
        }));
        let resolved = ResolvedEngine {
            engine: Engine::Named("agy".to_string()),
            team: None,
            wait: None,
        };
        let claude = vec![
            "claude".to_string(),
            "-p".to_string(),
            "việc".to_string(),
            "--append-system-prompt".to_string(),
            "luật".to_string(),
        ];
        let p_lead = run_plan(
            &cfg,
            &resolved,
            "lead",
            &claude,
            "m",
            BTreeMap::new(),
            Duration::from_secs(600),
        )
        .unwrap();
        let prompt_lead = &p_lead.argv[2];
        assert!(prompt_lead.contains("chế độ chỉ đọc"));
        assert!(prompt_lead.contains(
            "Bạn không chạy được lệnh shell (bị từ chối quyền); dùng ngữ cảnh đã cho và công cụ đọc file. Nếu thiếu thông tin, vẫn phải trả file đầy đủ, không được trả rỗng."
        ));

        let p_dev = run_plan(
            &cfg,
            &resolved,
            "dev",
            &claude,
            "m",
            BTreeMap::new(),
            Duration::from_secs(600),
        )
        .unwrap();
        let prompt_dev = &p_dev.argv[2];
        assert!(!prompt_dev.contains("chế độ chỉ đọc"));
        assert!(!prompt_dev.contains("Bạn không chạy được lệnh shell"));
    }
}
