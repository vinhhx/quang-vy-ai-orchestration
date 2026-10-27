//! Danh mục engine cố định (D-08): claude, deepseek, agy, người.
//! Lệnh, endpoint và biến key chỉ nằm trong config.json; ở đây chỉ có kiểu và độ sẵn sàng tính tại máy.

use crate::config::Config;
use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

pub const ENGINE_CLAUDE: &str = "claude";
pub const ENGINE_DEEPSEEK: &str = "deepseek";
pub const ENGINE_AGY: &str = "agy";
/// `runs.engine` of a human run (NFC).
pub const ENGINE_HUMAN: &str = "người";
pub const ENGINE_ORDER: [&str; 4] = ["claude", "deepseek", "agy", "người"];

const HUMAN_PREFIX: &str = "người:";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Engine {
    Claude,
    Deepseek,
    Agy,
    Human(String),
}

impl Engine {
    /// Trim + NFC, then `claude` | `deepseek` | `agy` | `người:<id>` with a valid staff id.
    pub fn parse(s: &str) -> Option<Engine> {
        let normalized: String = s.trim().nfc().collect();
        match normalized.as_str() {
            ENGINE_CLAUDE => Some(Engine::Claude),
            ENGINE_DEEPSEEK => Some(Engine::Deepseek),
            ENGINE_AGY => Some(Engine::Agy),
            other => {
                let id = other.strip_prefix(HUMAN_PREFIX)?;
                if crate::staff::validate_id(id) {
                    Some(Engine::Human(id.to_string()))
                } else {
                    None
                }
            }
        }
    }

    /// The value stored in `runs.engine`.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Engine::Claude => ENGINE_CLAUDE,
            Engine::Deepseek => ENGINE_DEEPSEEK,
            Engine::Agy => ENGINE_AGY,
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
const DESC_HUMAN: &str = "Nhân sự làm trên máy của họ và đẩy nhánh task lên remote.";

fn command_status(
    name: &str,
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
    }
}

/// Readiness of the four engines, in `ENGINE_ORDER`, from local checks only: a PATH
/// lookup and whether the key variable is set. No model call, and no env VALUE is ever
/// copied into the result (D-03).
pub fn readiness(
    cfg: &Config,
    env: &BTreeMap<String, String>,
    staff_count: usize,
) -> Vec<EngineStatus> {
    let path_var = env.get("PATH").map(String::as_str);

    let claude = command_status(ENGINE_CLAUDE, &cfg.claude_bin, path_var, DESC_CLAUDE);

    let ds = &cfg.engines.deepseek;
    let mut deepseek = command_status(ENGINE_DEEPSEEK, &ds.command, path_var, DESC_DEEPSEEK);
    deepseek.key_env = Some(ds.key_env.clone());
    let mut models: Vec<&str> = Vec::new();
    for role in crate::teams::ROLES {
        if let Some(m) = ds.models.get(role) {
            if !models.contains(&m.as_str()) {
                models.push(m);
            }
        }
    }
    deepseek.model = Some(models.join(", "));
    if deepseek.readiness == Readiness::SanSang.key() {
        let key_set = env.get(&ds.key_env).map_or(false, |v| !v.trim().is_empty());
        if !key_set {
            deepseek.readiness = Readiness::ThieuKey.key();
            deepseek.word = Readiness::ThieuKey.word();
            deepseek.reason = Some(format!("Chưa đặt biến môi trường {}.", ds.key_env));
        }
    }

    let mut agy = command_status(ENGINE_AGY, &cfg.engines.agy.command, path_var, DESC_AGY);
    agy.model = cfg.engines.agy.model.clone();

    let human = EngineStatus {
        name: ENGINE_HUMAN.to_string(),
        readiness: Readiness::SanSang.key(),
        word: Readiness::SanSang.word(),
        reason: None,
        command: None,
        key_env: None,
        model: None,
        description: DESC_HUMAN,
        detail: Some(format!("{staff_count} nhân sự")),
    };

    vec![claude, deepseek, agy, human]
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
}

/// What `execute_role_staged` spawns and records for one run.
#[derive(Debug, Clone)]
pub struct EnginePlan {
    pub engine: Engine,
    /// Value stored in `runs.engine`.
    pub db_engine: &'static str,
    /// Header word: `claude` | `deepseek` | `agy` | `người:<id>`.
    pub display: String,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub model_label: String,
    pub stream: StreamFormat,
    pub stdout_file: bool,
    /// Secret values to scrub from anything written to disk.
    pub redact: Vec<String>,
    pub response_file: Option<&'static str>,
}

/// `claude_argv` is the claude-family argv the role built; `base_env` is the project profile's
/// env (D-07: the claude engine always uses it unchanged). Other engines derive their own argv
/// from the claude one in later plans; until then they are refused here, before any run row.
pub fn plan_for(
    cfg: &Config,
    resolved: &ResolvedEngine,
    role: &str,
    claude_argv: &[String],
    claude_model: &str,
    base_env: BTreeMap<String, String>,
    timeout: Duration,
) -> BrainResult<EnginePlan> {
    match &resolved.engine {
        Engine::Claude => Ok(EnginePlan {
            engine: Engine::Claude,
            db_engine: ENGINE_CLAUDE,
            display: ENGINE_CLAUDE.to_string(),
            argv: claude_argv.to_vec(),
            env: base_env,
            model_label: claude_model.to_string(),
            stream: StreamFormat::Claude,
            stdout_file: false,
            redact: Vec::new(),
            response_file: None,
        }),
        Engine::Agy => agy_plan(cfg, role, claude_argv, base_env, timeout),
        Engine::Deepseek => Err(BrainError::error(format!(
            "Engine {} chưa được bật trong bản agent-brain này",
            resolved.engine.as_db_str()
        ))),
        Engine::Human(_) => Err(BrainError::error(
            "Engine người không chạy tiến trình — bước Dev của nhân sự bắt đầu bằng agent-brain run dev",
        )),
    }
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
            "{prompt}\n\n---\nBạn đang chạy ở chế độ chỉ đọc: không sửa và không tạo file nào. Trả lời bằng TOÀN BỘ nội dung file {file} theo đúng định dạng ở trên, không thêm lời dẫn, không bọc trong khối mã.\n\n---\n{input}"
        ),
        _ => format!("{prompt}\n\n---\n{input}"),
    };

    let acfg = &cfg.engines.agy;
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
        argv.extend(AGY_DEV_PERMISSION.iter().map(|s| s.to_string()));
    }

    // agy never sees claude or DeepSeek credentials or config (T-06-35).
    let mut env = base_env;
    env.retain(|k, _| {
        !(k.starts_with("ANTHROPIC_")
            || k == "CLAUDE_CONFIG_DIR"
            || k == "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"
            || k == "CLAUDE_CODE_SUBAGENT_MODEL"
            || k == &cfg.engines.deepseek.key_env)
    });

    Ok(EnginePlan {
        engine: Engine::Agy,
        db_engine: ENGINE_AGY,
        display: ENGINE_AGY.to_string(),
        argv,
        env,
        model_label: acfg.model.clone().unwrap_or_else(|| "mặc định".to_string()),
        stream: StreamFormat::Agy,
        stdout_file: true,
        redact: Vec::new(),
        response_file,
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
        let p = plan_for(
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

        for (engine, text) in [(
            Engine::Deepseek,
            "Engine deepseek chưa được bật trong bản agent-brain này",
        )] {
            let r = ResolvedEngine {
                engine,
                team: None,
                wait: None,
            };
            let e =
                plan_for(&cfg, &r, "dev", &argv, "m", BTreeMap::new(), Duration::ZERO).unwrap_err();
            assert_eq!(e.message, text);
        }
        let r = ResolvedEngine {
            engine: Engine::Human("ha".to_string()),
            team: None,
            wait: None,
        };
        assert!(plan_for(&cfg, &r, "dev", &argv, "m", BTreeMap::new(), Duration::ZERO).is_err());
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
            engine: Engine::Agy,
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
        let p = plan_for(
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

        let bad = plan_for(
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
            engine: Engine::Agy,
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
            let p = plan_for(
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
        let p = plan_for(
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
        for s in &all {
            assert_eq!(s.readiness, "san_sang", "{}", s.name);
            assert_eq!(s.word, "sẵn sàng");
            assert_eq!(s.reason, None);
        }
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
        assert_eq!(r[1].readiness, "san_sang");

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
        assert_eq!(Engine::parse("deepseek"), Some(Engine::Deepseek));
        assert_eq!(Engine::parse(" agy "), Some(Engine::Agy));
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
            "gpt",
            "",
            "người",
        ] {
            assert_eq!(Engine::parse(bad), None, "{bad:?} must not parse");
        }

        let h = Engine::Human("ha".to_string());
        assert_eq!(h.as_db_str(), "người");
        assert_eq!(h.to_config_string(), "người:ha");
        assert!(h.is_human());
        assert!(!Engine::Claude.is_human());
        assert_eq!(Engine::Deepseek.to_config_string(), "deepseek");

        let nfc: String = ENGINE_HUMAN.nfc().collect();
        assert_eq!(nfc, ENGINE_HUMAN, "ENGINE_HUMAN must be NFC");
        assert_eq!(ENGINE_ORDER, ["claude", "deepseek", "agy", "người"]);
    }

    #[test]
    fn readiness_keys_and_words() {
        assert_eq!(Readiness::SanSang.key(), "san_sang");
        assert_eq!(Readiness::ThieuKey.word(), "thiếu key");
        assert_eq!(Readiness::ThieuLenh.key(), "thieu_lenh");
        assert_eq!(Readiness::ChuaKiem.word(), "chưa kiểm");
    }
}
