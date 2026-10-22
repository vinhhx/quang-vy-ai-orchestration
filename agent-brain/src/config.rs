use crate::error::{BrainError, BrainResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_CONFIG_JSON: &str = include_str!("../assets/config.default.json");
pub const DEFAULT_DASHBOARD_PORT: u16 = 7437;
/// Telegram-created tasks allowed per local day when config.json has no
/// `telegram_max_tasks_per_day` (OD-2).
pub const DEFAULT_TELEGRAM_MAX_TASKS_PER_DAY: u32 = 10;
pub const REVIEW_MODEL_DEFAULT: &str = "opus";

#[derive(Debug, Clone, PartialEq)]
pub struct AutopilotDefaults {
    pub max_rounds: u32,
    pub budget_minutes: u32,
    pub budget_usd: f64,
}

impl Default for AutopilotDefaults {
    fn default() -> Self {
        Self {
            max_rounds: 3,
            budget_minutes: 60,
            budget_usd: 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

pub fn default_pricing() -> BTreeMap<String, Price> {
    let mut m = BTreeMap::new();
    m.insert(
        "opus".to_string(),
        Price {
            input: 4.0,
            output: 20.0,
            cache_read: 0.2,
            cache_write: 8.0,
        },
    );
    m.insert(
        "sonnet".to_string(),
        Price {
            input: 2.0,
            output: 10.0,
            cache_read: 0.1,
            cache_write: 4.0,
        },
    );
    m.insert(
        "haiku".to_string(),
        Price {
            input: 0.1,
            output: 0.5,
            cache_read: 0.005,
            cache_write: 0.2,
        },
    );
    m
}

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
    pub autopilot: AutopilotDefaults,
    pub pricing: BTreeMap<String, Price>,
    /// Tasks the Telegram bot may create per local day; 0 turns `/task` off.
    pub telegram_max_tasks_per_day: u32,
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
    models
        .entry("plan-review".to_string())
        .or_insert_with(|| REVIEW_MODEL_DEFAULT.to_string());
    models
        .entry("code-review".to_string())
        .or_insert_with(|| REVIEW_MODEL_DEFAULT.to_string());

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
    // IN-01 (D-07): a config written before 5.2 has no review timeouts; give the
    // reviewers their intended ceilings instead of the 1800 s fallback.
    timeout_seconds
        .entry("plan-review".to_string())
        .or_insert(1200);
    timeout_seconds
        .entry("code-review".to_string())
        .or_insert(1500);

    let mut autopilot = AutopilotDefaults::default();
    if let Some(ap_val) = obj.get("autopilot") {
        if let Some(ap_obj) = ap_val.as_object() {
            if let Some(mr) = ap_obj.get("max_rounds") {
                match mr.as_u64() {
                    Some(n) if (1..=10).contains(&n) => autopilot.max_rounds = n as u32,
                    _ => {
                        return Err(BrainError::error(
                            "Cấu hình autopilot.max_rounds phải là số nguyên từ 1 đến 10",
                        ))
                    }
                }
            }
            if let Some(bm) = ap_obj.get("budget_minutes") {
                match bm.as_u64() {
                    Some(n) if (1..=1440).contains(&n) => autopilot.budget_minutes = n as u32,
                    _ => {
                        return Err(BrainError::error(
                            "Cấu hình autopilot.budget_minutes phải là số nguyên từ 1 đến 1440",
                        ))
                    }
                }
            }
            if let Some(bu) = ap_obj.get("budget_usd") {
                match bu.as_f64() {
                    Some(f) if f.is_finite() && f > 0.0 && f <= 1000.0 => autopilot.budget_usd = f,
                    _ => {
                        return Err(BrainError::error(
                            "Cấu hình autopilot.budget_usd phải là số lớn hơn 0 và không quá 1000",
                        ))
                    }
                }
            }
        }
    }

    let telegram_max_tasks_per_day = match obj.get("telegram_max_tasks_per_day") {
        None => DEFAULT_TELEGRAM_MAX_TASKS_PER_DAY,
        Some(val) => match val.as_u64() {
            Some(n) if n <= 1000 => n as u32,
            _ => {
                return Err(BrainError::error(
                    "Cấu hình telegram_max_tasks_per_day phải là số nguyên từ 0 đến 1000",
                ))
            }
        },
    };

    let mut pricing = default_pricing();
    if let Some(p_val) = obj.get("pricing") {
        if let Some(p_obj) = p_val.as_object() {
            for (family, val) in p_obj {
                let price: Price = serde_json::from_value(val.clone()).map_err(|_| {
                    BrainError::error(format!(
                        "Cấu hình pricing.{family} không hợp lệ (cần input, output, cache_read, cache_write là số không âm)"
                    ))
                })?;
                if !price.input.is_finite()
                    || price.input < 0.0
                    || !price.output.is_finite()
                    || price.output < 0.0
                    || !price.cache_read.is_finite()
                    || price.cache_read < 0.0
                    || !price.cache_write.is_finite()
                    || price.cache_write < 0.0
                {
                    return Err(BrainError::error(format!(
                        "Cấu hình pricing.{family} không hợp lệ (cần input, output, cache_read, cache_write là số không âm)"
                    )));
                }
                pricing.insert(family.clone(), price);
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
        autopilot,
        pricing,
        telegram_max_tasks_per_day,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_review_models_and_limits() {
        let val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let cfg = config_from_value(&val).unwrap();
        assert_eq!(cfg.model_for("plan-review").unwrap(), "opus");
        assert_eq!(cfg.model_for("code-review").unwrap(), "opus");
        assert_eq!(cfg.timeout_for("plan-review"), Duration::from_secs(1200));
        assert_eq!(cfg.timeout_for("code-review"), Duration::from_secs(1500));
        assert_eq!(
            cfg.autopilot,
            AutopilotDefaults {
                max_rounds: 3,
                budget_minutes: 60,
                budget_usd: 20.0,
            }
        );
        assert_eq!(
            cfg.pricing.get("opus").copied().unwrap(),
            Price {
                input: 4.0,
                output: 20.0,
                cache_read: 0.2,
                cache_write: 8.0,
            }
        );
        assert_eq!(cfg.pricing.get("sonnet").unwrap().cache_write, 4.0);
        assert_eq!(cfg.pricing.get("haiku").unwrap().input, 0.1);
    }

    #[test]
    fn minimal_config_gets_defaults() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let val = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "profiles": default_val["profiles"]
        });
        let cfg = config_from_value(&val).unwrap();
        assert_eq!(cfg.model_for("plan-review").unwrap(), "opus");
        assert_eq!(cfg.model_for("code-review").unwrap(), "opus");
        assert_eq!(cfg.autopilot, AutopilotDefaults::default());
        assert_eq!(cfg.pricing, default_pricing());
    }

    #[test]
    fn review_timeouts_default_when_missing() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let val = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "profiles": default_val["profiles"],
            "timeout_seconds": { "lead": 1800 }
        });
        let cfg = config_from_value(&val).unwrap();
        assert_eq!(cfg.timeout_for("plan-review"), Duration::from_secs(1200));
        assert_eq!(cfg.timeout_for("code-review"), Duration::from_secs(1500));
        assert_eq!(cfg.timeout_for("dev"), Duration::from_secs(1800));
    }

    #[test]
    fn review_timeouts_explicit_win() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let val = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "profiles": default_val["profiles"],
            "timeout_seconds": { "lead": 1800, "plan-review": 900 }
        });
        let cfg = config_from_value(&val).unwrap();
        assert_eq!(cfg.timeout_for("plan-review"), Duration::from_secs(900));
        assert_eq!(cfg.timeout_for("code-review"), Duration::from_secs(1500));
    }

    #[test]
    fn review_model_override() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let val = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet",
                "plan-review": "sonnet"
            },
            "profiles": default_val["profiles"]
        });
        let cfg = config_from_value(&val).unwrap();
        assert_eq!(cfg.model_for("plan-review").unwrap(), "sonnet");
        assert_eq!(cfg.model_for("code-review").unwrap(), "opus");
    }

    #[test]
    fn autopilot_override_and_ranges() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let base = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "profiles": default_val["profiles"]
        });

        // override max_rounds: 5
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"max_rounds": 5});
        let cfg = config_from_value(&v).unwrap();
        assert_eq!(cfg.autopilot.max_rounds, 5);
        assert_eq!(cfg.autopilot.budget_minutes, 60);
        assert_eq!(cfg.autopilot.budget_usd, 20.0);

        // max_rounds 0 -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"max_rounds": 0});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.max_rounds phải là số nguyên từ 1 đến 10"
        );

        // max_rounds 11 -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"max_rounds": 11});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.max_rounds phải là số nguyên từ 1 đến 10"
        );

        // budget_minutes 0 -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"budget_minutes": 0});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.budget_minutes phải là số nguyên từ 1 đến 1440"
        );

        // budget_usd 0 -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"budget_usd": 0});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.budget_usd phải là số lớn hơn 0 và không quá 1000"
        );

        // budget_usd 1001 -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"budget_usd": 1001});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.budget_usd phải là số lớn hơn 0 và không quá 1000"
        );

        // budget_usd "x" -> Err
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!({"budget_usd": "x"});
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình autopilot.budget_usd phải là số lớn hơn 0 và không quá 1000"
        );

        // autopilot not an object -> defaults
        let mut v = base.clone();
        v["autopilot"] = serde_json::json!("not-an-object");
        let cfg = config_from_value(&v).unwrap();
        assert_eq!(cfg.autopilot, AutopilotDefaults::default());
    }

    #[test]
    fn pricing_override_one_family() {
        let default_val: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        let base = serde_json::json!({
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "profiles": default_val["profiles"]
        });

        let mut v = base.clone();
        v["pricing"] = serde_json::json!({
            "sonnet": {
                "input": 3,
                "output": 15,
                "cache_read": 0.3,
                "cache_write": 6
            }
        });
        let cfg = config_from_value(&v).unwrap();
        assert_eq!(
            cfg.pricing.get("sonnet").copied().unwrap(),
            Price {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 6.0,
            }
        );
        assert_eq!(
            cfg.pricing.get("opus").copied().unwrap(),
            default_pricing()["opus"]
        );
        assert_eq!(
            cfg.pricing.get("haiku").copied().unwrap(),
            default_pricing()["haiku"]
        );

        // missing field
        let mut v = base.clone();
        v["pricing"] = serde_json::json!({
            "sonnet": {
                "input": 3,
                "output": 15
            }
        });
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình pricing.sonnet không hợp lệ (cần input, output, cache_read, cache_write là số không âm)"
        );

        // negative field
        let mut v = base.clone();
        v["pricing"] = serde_json::json!({
            "sonnet": {
                "input": -1.0,
                "output": 15,
                "cache_read": 0.3,
                "cache_write": 6
            }
        });
        let err = config_from_value(&v).unwrap_err();
        assert_eq!(
            err.message,
            "Cấu hình pricing.sonnet không hợp lệ (cần input, output, cache_read, cache_write là số không âm)"
        );
    }

    const TELEGRAM_CAP_ERR: &str =
        "Cấu hình telegram_max_tasks_per_day phải là số nguyên từ 0 đến 1000";

    #[test]
    fn telegram_max_tasks_per_day_default_and_range() {
        let base: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        assert!(base.get("telegram_max_tasks_per_day").is_none());
        assert_eq!(
            config_from_value(&base).unwrap().telegram_max_tasks_per_day,
            10
        );
        for n in [0u32, 1, 1000] {
            let mut v = base.clone();
            v["telegram_max_tasks_per_day"] = serde_json::json!(n);
            assert_eq!(
                config_from_value(&v).unwrap().telegram_max_tasks_per_day,
                n,
                "{n}"
            );
        }
        for bad in [
            serde_json::json!(1001),
            serde_json::json!(-1),
            serde_json::json!("5"),
            serde_json::json!(2.5),
            serde_json::json!(null),
        ] {
            let mut v = base.clone();
            v["telegram_max_tasks_per_day"] = bad.clone();
            let err = config_from_value(&v).unwrap_err();
            assert_eq!(err.message, TELEGRAM_CAP_ERR, "{bad}");
        }
    }
}
