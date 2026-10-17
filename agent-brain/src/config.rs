use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_CONFIG_JSON: &str = include_str!("../assets/config.default.json");
pub const DEFAULT_DASHBOARD_PORT: u16 = 7437;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProfileCfg {
    pub claude_config_dir: Option<String>,
    #[serde(default)]
    pub unset_env: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub claude_bin: String,
    pub models: BTreeMap<String, String>,
    pub inspect_model: String,
    pub profiles: BTreeMap<String, ProfileCfg>,
    pub common_args: Vec<String>,
    pub max_turns: BTreeMap<String, Option<u32>>,
    pub timeout_seconds: BTreeMap<String, u64>,
    pub runs_dir: Option<String>,
    pub kill_grace_secs: u64,
    pub inspect_max_diff_bytes: usize,
    pub dashboard_port: u16,
}

impl Config {
    pub fn timeout_for(&self, role: &str) -> Duration {
        self.timeout_seconds
            .get(role)
            .copied()
            .map(Duration::from_secs)
            .unwrap_or_else(|| {
                if role == "inspect" {
                    Duration::from_secs(300)
                } else {
                    Duration::from_secs(1800)
                }
            })
    }

    pub fn model_for(&self, role: &str) -> BrainResult<String> {
        self.models
            .get(role)
            .cloned()
            .ok_or_else(|| BrainError::error(format!("Cấu hình thiếu model cho vai trò '{role}'")))
    }
}

pub fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    PathBuf::from("/")
}

pub fn expand_tilde(s: &str, home: &Path) -> PathBuf {
    if s == "~" {
        home.to_path_buf()
    } else if let Some(stripped) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        home.join(stripped)
    } else {
        PathBuf::from(s)
    }
}

pub fn config_dir() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(v) if !v.is_empty() => PathBuf::from(v).join("agent-brain"),
        _ => home_dir().join(".config").join("agent-brain"),
    }
}

pub fn data_dir() -> PathBuf {
    match std::env::var("XDG_DATA_HOME") {
        Ok(v) if !v.is_empty() => PathBuf::from(v).join("agent-brain"),
        _ => home_dir().join(".local").join("share").join("agent-brain"),
    }
}

pub fn map_path() -> PathBuf {
    config_dir().join("projects_map.json")
}

pub fn db_path() -> PathBuf {
    match std::env::var("AGENT_BRAIN_DB") {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => data_dir().join("brain.db"),
    }
}

pub fn runs_dir(cfg: &Config) -> PathBuf {
    if let Some(ref rd) = cfg.runs_dir {
        expand_tilde(rd, &home_dir())
    } else {
        data_dir().join("runs")
    }
}

pub fn config_from_value(v: &serde_json::Value) -> BrainResult<Config> {
    let obj = v
        .as_object()
        .ok_or_else(|| BrainError::error("Cấu hình không hợp lệ (phải là JSON object)"))?;

    let models_val = obj
        .get("models")
        .and_then(|m| m.as_object())
        .ok_or_else(|| BrainError::error("Cấu hình thiếu trường 'models'"))?;

    for r in ["lead", "dev", "qa"] {
        let val_opt = models_val.get(r).and_then(|val| val.as_str());
        match val_opt {
            Some(s) if !s.trim().is_empty() => {}
            _ => {
                return Err(BrainError::error(format!(
                    "Cấu hình thiếu model cho vai trò '{r}'"
                )))
            }
        }
    }

    let profiles_val = obj
        .get("profiles")
        .and_then(|p| p.as_object())
        .ok_or_else(|| BrainError::error("Cấu hình thiếu trường 'profiles'"))?;

    for p in ["work", "personal"] {
        if !profiles_val.contains_key(p) {
            return Err(BrainError::error(format!("Cấu hình thiếu profile '{p}'")));
        }
    }

    let claude_bin = obj
        .get("claude_bin")
        .and_then(|s| s.as_str())
        .unwrap_or("claude")
        .to_string();

    let inspect_model = obj
        .get("inspect_model")
        .and_then(|s| s.as_str())
        .unwrap_or("sonnet")
        .to_string();

    let kill_grace_secs = obj
        .get("kill_grace_secs")
        .and_then(|n| n.as_u64())
        .unwrap_or(3);

    let inspect_max_diff_bytes = obj
        .get("inspect_max_diff_bytes")
        .and_then(|n| n.as_u64())
        .map(|n| n as usize)
        .unwrap_or(100_000);

    let dashboard_port = match obj.get("dashboard_port") {
        Some(val) => match val.as_u64() {
            Some(n) if n <= 65535 => n as u16,
            _ => {
                return Err(BrainError::error(format!(
                    "Cấu hình dashboard_port không hợp lệ: {val}"
                )))
            }
        },
        None => DEFAULT_DASHBOARD_PORT,
    };

    let common_args = obj
        .get("common_args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let runs_dir_opt = obj
        .get("runs_dir")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let mut models = BTreeMap::new();
    for (k, val) in models_val {
        if let Some(s) = val.as_str() {
            models.insert(k.clone(), s.to_string());
        }
    }

    let mut profiles = BTreeMap::new();
    for (k, val) in profiles_val {
        if let Ok(pcfg) = serde_json::from_value::<ProfileCfg>(val.clone()) {
            profiles.insert(k.clone(), pcfg);
        }
    }

    let mut max_turns = BTreeMap::new();
    if let Some(mt_obj) = obj.get("max_turns").and_then(|m| m.as_object()) {
        for (k, val) in mt_obj {
            let turn_val = val.as_u64().map(|n| n as u32);
            max_turns.insert(k.clone(), turn_val);
        }
    }

    let mut timeout_seconds = BTreeMap::new();
    if let Some(ts_obj) = obj.get("timeout_seconds").and_then(|m| m.as_object()) {
        for (k, val) in ts_obj {
            if let Some(secs) = val.as_u64() {
                timeout_seconds.insert(k.clone(), secs);
            }
        }
    }

    Ok(Config {
        claude_bin,
        models,
        inspect_model,
        profiles,
        common_args,
        max_turns,
        timeout_seconds,
        runs_dir: runs_dir_opt,
        kill_grace_secs,
        inspect_max_diff_bytes,
        dashboard_port,
    })
}

pub fn load_config() -> BrainResult<Config> {
    if let Ok(cfg_env) = std::env::var("AGENT_BRAIN_CONFIG") {
        if !cfg_env.is_empty() {
            let path = expand_tilde(&cfg_env, &home_dir());
            if !path.is_file() {
                return Err(BrainError::error(format!(
                    "Không tìm thấy file cấu hình: {}",
                    path.display()
                )));
            }
            let content = std::fs::read_to_string(&path).map_err(|e| {
                BrainError::error(format!("Lỗi đọc cấu hình {}: {e}", path.display()))
            })?;
            let val = serde_json::from_str(&content).map_err(|e| {
                BrainError::error(format!("Lỗi đọc cấu hình {}: {e}", path.display()))
            })?;
            return config_from_value(&val);
        }
    }

    let cfg_file = config_dir().join("config.json");
    if cfg_file.is_file() {
        let content = std::fs::read_to_string(&cfg_file).map_err(|e| {
            BrainError::error(format!("Lỗi đọc cấu hình {}: {e}", cfg_file.display()))
        })?;
        let val = serde_json::from_str(&content).map_err(|e| {
            BrainError::error(format!("Lỗi đọc cấu hình {}: {e}", cfg_file.display()))
        })?;
        return config_from_value(&val);
    }

    let default_val = serde_json::from_str(DEFAULT_CONFIG_JSON)
        .map_err(|e| BrainError::error(format!("Lỗi đọc cấu hình mặc định: {e}")))?;
    config_from_value(&default_val)
}
