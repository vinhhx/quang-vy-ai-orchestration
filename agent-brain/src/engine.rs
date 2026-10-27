//! Danh mục engine cố định (D-08): claude, deepseek, agy, người.
//! Lệnh, endpoint và biến key chỉ nằm trong config.json; ở đây chỉ có kiểu và độ sẵn sàng tính tại máy.

use crate::config::Config;
use std::collections::BTreeMap;
use std::path::Path;
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

#[cfg(test)]
mod tests {
    use super::*;

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
