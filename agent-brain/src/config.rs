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

/// The three kinds of a configured engine (D-30). An OpenAI-format kind is deferred (D-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Anthropic,
    Agy,
    Dsh,
}

impl EngineKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EngineKind::Anthropic => "anthropic",
            EngineKind::Agy => "agy",
            EngineKind::Dsh => "dsh",
        }
    }
}

/// `claude` on the host, pointed at an Anthropic-format endpoint (`base_url`) or, without
/// `base_url`, on the project's own profile (a native entry).
#[derive(Debug, Clone, PartialEq)]
pub struct AnthropicDef {
    /// Executable only; agent-brain builds every argument itself (D-41).
    pub command: String,
    pub base_url: Option<String>,
    /// NAME of the environment variable that holds the key; the value is never read into Config (D-03).
    pub key_env: Option<String>,
    /// Model per role (`lead`, `dev`, `qa`, `plan-review`, `code-review`).
    pub models: BTreeMap<String, String>,
    /// Native entries only: the profile this entry is tied to (implies `allowed_profiles`).
    pub pin_profile: Option<String>,
    /// Compatibility knobs of an endpoint: `disable_experimental_betas`, `disable_adaptive_thinking`.
    pub compat: Vec<String>,
    /// Free text recording the E-09 evidence for the endpoint (D-40).
    pub verified: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgyDef {
    pub command: String,
    pub print_timeout_margin_secs: u64,
    pub model: Option<String>,
    pub dev_permission: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DshDef {
    pub docker: String,
    pub container: String,
    pub dsh_profile: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineSpec {
    Anthropic(AnthropicDef),
    Agy(AgyDef),
    Dsh(DshDef),
}

/// One entry of `config.json` `engines`, keyed by `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineDef {
    pub id: String,
    pub label: Option<String>,
    pub allowed_profiles: Option<Vec<String>>,
    pub pricing: Option<Price>,
    pub spec: EngineSpec,
}

impl EngineDef {
    pub fn kind(&self) -> EngineKind {
        match &self.spec {
            EngineSpec::Anthropic(_) => EngineKind::Anthropic,
            EngineSpec::Agy(_) => EngineKind::Agy,
            EngineSpec::Dsh(_) => EngineKind::Dsh,
        }
    }

    pub fn anthropic(&self) -> Option<&AnthropicDef> {
        match &self.spec {
            EngineSpec::Anthropic(a) => Some(a),
            _ => None,
        }
    }

    pub fn agy(&self) -> Option<&AgyDef> {
        match &self.spec {
            EngineSpec::Agy(a) => Some(a),
            _ => None,
        }
    }

    pub fn dsh(&self) -> Option<&DshDef> {
        match &self.spec {
            EngineSpec::Dsh(d) => Some(d),
            _ => None,
        }
    }

    /// An `anthropic` entry without `base_url`: runs on the project's profile, injects nothing.
    pub fn is_native(&self) -> bool {
        matches!(&self.spec, EngineSpec::Anthropic(a) if a.base_url.is_none())
    }

    /// NAME of the key variable (an `anthropic` entry with `base_url`).
    pub fn key_env(&self) -> Option<&str> {
        self.anthropic().and_then(|a| a.key_env.as_deref())
    }

    /// `[pin_profile]` for a pinned entry, else `allowed_profiles`; `None` = no restriction.
    pub fn effective_allowed_profiles(&self) -> Option<Vec<String>> {
        if let Some(p) = self.anthropic().and_then(|a| a.pin_profile.as_ref()) {
            return Some(vec![p.clone()]);
        }
        self.allowed_profiles.clone()
    }
}

/// The user's engine list, in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct EnginesCfg {
    pub list: Vec<EngineDef>,
}

impl EnginesCfg {
    /// The two historical entries, used when `engines` is absent or in the legacy object shape.
    pub fn defaults(claude_bin: &str) -> EnginesCfg {
        EnginesCfg {
            list: vec![
                EngineDef {
                    id: "deepseek".to_string(),
                    label: None,
                    allowed_profiles: None,
                    pricing: None,
                    spec: EngineSpec::Anthropic(AnthropicDef {
                        command: claude_bin.to_string(),
                        base_url: Some(DEFAULT_DEEPSEEK_BASE_URL.to_string()),
                        key_env: Some(DEFAULT_DEEPSEEK_KEY_ENV.to_string()),
                        models: default_deepseek_models(),
                        pin_profile: None,
                        compat: Vec::new(),
                        verified: None,
                    }),
                },
                EngineDef {
                    id: "agy".to_string(),
                    label: None,
                    allowed_profiles: None,
                    pricing: None,
                    spec: EngineSpec::Agy(AgyDef {
                        command: DEFAULT_AGY_COMMAND.to_string(),
                        print_timeout_margin_secs: DEFAULT_AGY_PRINT_TIMEOUT_MARGIN_SECS,
                        model: None,
                        dev_permission: DEFAULT_AGY_DEV_PERMISSION.to_string(),
                    }),
                },
            ],
        }
    }

    pub fn get(&self, id: &str) -> Option<&EngineDef> {
        self.list.iter().find(|e| e.id == id)
    }

    /// `None` for an id that is not in the list; never panics.
    pub fn kind_of(&self, id: &str) -> Option<EngineKind> {
        self.get(id).map(EngineDef::kind)
    }

    /// Every key variable NAME of every entry, sorted and deduplicated. Every engine's child env
    /// has these removed (Pitfall A2).
    pub fn all_key_envs(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .list
            .iter()
            .filter_map(|e| e.key_env().map(str::to_string))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn ids(&self) -> Vec<&str> {
        self.list.iter().map(|e| e.id.as_str()).collect()
    }
}

pub const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/anthropic";
pub const DEFAULT_DEEPSEEK_KEY_ENV: &str = "DEEPSEEK_API_KEY";
pub const DEFAULT_AGY_COMMAND: &str = "agy";
pub const DEFAULT_AGY_PRINT_TIMEOUT_MARGIN_SECS: u64 = 30;
pub const DEFAULT_AGY_DEV_PERMISSION: &str = "accept-edits";
pub const DEFAULT_DSH_DOCKER: &str = "docker";
pub const DEFAULT_DSH_PROFILE: &str = "headless";

/// Role aliases the deepseek engine uses until a run decides otherwise (plan 06-11).
pub fn default_deepseek_models() -> BTreeMap<String, String> {
    [
        ("lead", "opus"),
        ("dev", "sonnet"),
        ("qa", "sonnet"),
        ("plan-review", "opus"),
        ("code-review", "opus"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn valid_key_env(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// A one-line, non-empty text of at most `max` characters.
fn one_line_text(s: &str, max: usize) -> Option<String> {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > max || t.contains('\n') || t.contains('\r') {
        None
    } else {
        Some(t.to_string())
    }
}

/// An executable only: a string, or an array with exactly one string (D-41). Extra elements would
/// become argv, which only agent-brain may build.
fn parse_command_field(v: &serde_json::Value, label: &str, field: &str) -> BrainResult<String> {
    let empty = || BrainError::error(format!("Cấu hình {label}.{field} không được rỗng"));
    let one = |s: Option<&str>| match s.map(str::trim) {
        Some(s) if !s.is_empty() => Ok(s.to_string()),
        _ => Err(empty()),
    };
    match v {
        serde_json::Value::Array(a) => match a.len() {
            0 => Err(empty()),
            1 => one(a[0].as_str()),
            _ => Err(BrainError::error(format!(
                "Cấu hình {label}.{field} chỉ nhận tên lệnh, không nhận tham số"
            ))),
        },
        other => one(other.as_str()),
    }
}

fn parse_verified(v: &serde_json::Value, label: &str) -> BrainResult<String> {
    v.as_str()
        .and_then(|s| one_line_text(s, 200))
        .ok_or_else(|| {
            BrainError::error(format!(
                "Cấu hình {label}.verified không hợp lệ (một dòng, tối đa 200 ký tự)"
            ))
        })
}

fn parse_price(v: &serde_json::Value, label: &str) -> BrainResult<Price> {
    let bad = || {
        BrainError::error(format!(
            "Cấu hình {label}.pricing không hợp lệ (cần input, output, cache_read, cache_write là số không âm)"
        ))
    };
    let p: Price = serde_json::from_value(v.clone()).map_err(|_| bad())?;
    for n in [p.input, p.output, p.cache_read, p.cache_write] {
        if !n.is_finite() || n < 0.0 {
            return Err(bad());
        }
    }
    Ok(p)
}

/// The `command` / `print_timeout_margin_secs` / `model` rules of an agy entry, shared by the
/// legacy object shape and the array shape (same texts, `{label}` = `engines.agy` or `engines.{id}`).
fn apply_agy_fields(
    obj: &serde_json::Map<String, serde_json::Value>,
    label: &str,
    agy: &mut AgyDef,
) -> BrainResult<()> {
    if let Some(c) = obj.get("command").filter(|c| !c.is_null()) {
        agy.command = parse_command_field(c, label, "command")?;
    }
    if let Some(n) = obj
        .get("print_timeout_margin_secs")
        .filter(|n| !n.is_null())
    {
        match n.as_u64() {
            Some(n) if n <= 3600 => agy.print_timeout_margin_secs = n,
            _ => {
                return Err(BrainError::error(format!(
                    "Cấu hình {label}.print_timeout_margin_secs phải là số nguyên từ 0 đến 3600"
                )))
            }
        }
    }
    if let Some(m) = obj.get("model").filter(|m| !m.is_null()) {
        match m.as_str().map(str::trim) {
            Some(s) if !s.is_empty() => agy.model = Some(s.to_string()),
            _ => {
                return Err(BrainError::error(format!(
                    "Cấu hình {label}.model không hợp lệ"
                )))
            }
        }
    }
    Ok(())
}

/// `engines` of config.json: absent/null → the two default entries; the legacy object
/// `{deepseek, agy}` → the same two entries with the given values; an array → the user's list
/// exactly as written (never merged with the defaults). No file is ever rewritten.
pub fn parse_engines(
    v: Option<&serde_json::Value>,
    claude_bin: &str,
    profile_names: &[String],
) -> BrainResult<EnginesCfg> {
    match v {
        None | Some(serde_json::Value::Null) => Ok(EnginesCfg::defaults(claude_bin)),
        Some(serde_json::Value::Object(o)) => parse_engines_object(o, claude_bin),
        Some(serde_json::Value::Array(a)) => parse_engines_array(a, claude_bin, profile_names),
        Some(_) => Err(BrainError::error(
            "Cấu hình engines không hợp lệ (phải là mảng hoặc JSON object)",
        )),
    }
}

/// The legacy object shape `{"deepseek": {...}, "agy": {...}}` (rules and texts unchanged).
fn parse_engines_object(
    engines: &serde_json::Map<String, serde_json::Value>,
    claude_bin: &str,
) -> BrainResult<EnginesCfg> {
    let mut cfg = EnginesCfg::defaults(claude_bin);

    for key in engines.keys() {
        if key != "deepseek" && key != "agy" {
            return Err(BrainError::error(format!(
                "Cấu hình engines.{key} không hợp lệ (chỉ deepseek, agy)"
            )));
        }
    }

    if let Some(d) = engines.get("deepseek").filter(|d| !d.is_null()) {
        let d = d.as_object().ok_or_else(|| {
            BrainError::error("Cấu hình engines.deepseek không hợp lệ (phải là JSON object)")
        })?;
        let EngineSpec::Anthropic(ds) = &mut cfg.list[0].spec else {
            unreachable!("defaults() lists deepseek first");
        };
        if let Some(c) = d.get("command").filter(|c| !c.is_null()) {
            ds.command = parse_command_field(c, "engines.deepseek", "command")?;
        }
        if let Some(u) = d.get("base_url").filter(|u| !u.is_null()) {
            match u.as_str().map(str::trim) {
                Some(s) if s.starts_with("https://") => ds.base_url = Some(s.to_string()),
                _ => {
                    return Err(BrainError::error(
                        "Cấu hình engines.deepseek.base_url phải bắt đầu bằng https://",
                    ))
                }
            }
        }
        if let Some(k) = d.get("key_env").filter(|k| !k.is_null()) {
            match k.as_str() {
                Some(s) if valid_key_env(s) => ds.key_env = Some(s.to_string()),
                _ => {
                    return Err(BrainError::error(
                        "Cấu hình engines.deepseek.key_env không hợp lệ (chỉ chữ hoa, số và _)",
                    ))
                }
            }
        }
        if let Some(m) = d.get("models").filter(|m| !m.is_null()) {
            let m = m.as_object().ok_or_else(|| {
                BrainError::error(
                    "Cấu hình engines.deepseek.models không hợp lệ (phải là JSON object)",
                )
            })?;
            for (role, val) in m {
                let known = ds.models.contains_key(role);
                match val.as_str().map(str::trim) {
                    Some(s) if known && !s.is_empty() => {
                        ds.models.insert(role.clone(), s.to_string());
                    }
                    _ => {
                        return Err(BrainError::error(format!(
                            "Cấu hình engines.deepseek.models.{role} không hợp lệ"
                        )))
                    }
                }
            }
        }
        if let Some(v) = d.get("verified").filter(|v| !v.is_null()) {
            ds.verified = Some(parse_verified(v, "engines.deepseek")?);
        }
    }

    if let Some(a) = engines.get("agy").filter(|a| !a.is_null()) {
        let a = a.as_object().ok_or_else(|| {
            BrainError::error("Cấu hình engines.agy không hợp lệ (phải là JSON object)")
        })?;
        let EngineSpec::Agy(agy) = &mut cfg.list[1].spec else {
            unreachable!("defaults() lists agy second");
        };
        apply_agy_fields(a, "engines.agy", agy)?;
    }

    Ok(cfg)
}

const COMMON_ENGINE_KEYS: [&str; 5] = ["id", "kind", "label", "allowed_profiles", "pricing"];
const ANTHROPIC_ENGINE_KEYS: [&str; 7] = [
    "command",
    "base_url",
    "key_env",
    "models",
    "pin_profile",
    "compat",
    "verified",
];
const AGY_ENGINE_KEYS: [&str; 4] = [
    "command",
    "print_timeout_margin_secs",
    "model",
    "dev_permission",
];
const DSH_ENGINE_KEYS: [&str; 3] = ["docker", "container", "dsh_profile"];
const COMPAT_KNOBS: [&str; 2] = ["disable_experimental_betas", "disable_adaptive_thinking"];

/// The array shape: one entry per engine, in file order.
fn parse_engines_array(
    arr: &[serde_json::Value],
    claude_bin: &str,
    profile_names: &[String],
) -> BrainResult<EnginesCfg> {
    let mut list: Vec<EngineDef> = Vec::new();
    for (i, v) in arr.iter().enumerate() {
        let def = parse_engine_entry(i, v, &list, claude_bin, profile_names)?;
        list.push(def);
    }
    Ok(EnginesCfg { list })
}

fn parse_engine_entry(
    i: usize,
    v: &serde_json::Value,
    earlier: &[EngineDef],
    claude_bin: &str,
    profile_names: &[String],
) -> BrainResult<EngineDef> {
    use unicode_normalization::UnicodeNormalization;

    let obj = v.as_object().ok_or_else(|| {
        BrainError::error(format!(
            "Cấu hình engines[{i}] không hợp lệ (phải là JSON object)"
        ))
    })?;

    // id: built-ins first (any normalisation), then the name rule, then duplicates.
    let bad_id = || {
        BrainError::error(format!(
            "Cấu hình engines[{i}].id không hợp lệ (chữ thường không dấu, số và -, tối đa 32 ký tự)"
        ))
    };
    let raw_id = obj.get("id").and_then(|x| x.as_str()).ok_or_else(bad_id)?;
    let normalized: String = raw_id.trim().nfc().collect();
    if normalized == crate::engine::ENGINE_CLAUDE || normalized == crate::engine::ENGINE_HUMAN {
        return Err(BrainError::error(format!(
            "Cấu hình engines[{i}].id '{raw_id}' là engine có sẵn — xóa mục này khỏi danh sách engines."
        )));
    }
    if !crate::workflows::validate_name(raw_id) || raw_id.len() > 32 {
        return Err(bad_id());
    }
    let id = raw_id.to_string();
    if let Some(j) = earlier.iter().position(|e| e.id == id) {
        return Err(BrainError::error(format!(
            "Cấu hình engines.{id} bị khai báo hai lần (engines[{j}] và engines[{i}])"
        )));
    }
    let label = format!("engines.{id}");
    let err = |msg: String| BrainError::error(format!("Cấu hình {label}{msg}"));

    let kind = match obj.get("kind").and_then(|k| k.as_str()) {
        Some("anthropic") => EngineKind::Anthropic,
        Some("agy") => EngineKind::Agy,
        Some("dsh") => EngineKind::Dsh,
        _ => {
            return Err(err(
                ".kind không hợp lệ (chỉ anthropic, agy, dsh)".to_string()
            ))
        }
    };

    // No key outside the list for the kind: nothing else may ever reach argv or env (A1, A3).
    let kind_keys: &[&str] = match kind {
        EngineKind::Anthropic => &ANTHROPIC_ENGINE_KEYS,
        EngineKind::Agy => &AGY_ENGINE_KEYS,
        EngineKind::Dsh => &DSH_ENGINE_KEYS,
    };
    for k in obj.keys() {
        if k.starts_with('_') {
            continue;
        }
        if !COMMON_ENGINE_KEYS.contains(&k.as_str()) && !kind_keys.contains(&k.as_str()) {
            return Err(err(format!(": khóa lạ {k}")));
        }
    }

    let label_text = match obj.get("label").filter(|l| !l.is_null()) {
        None => None,
        Some(l) => Some(
            l.as_str()
                .and_then(|s| one_line_text(s, 120))
                .ok_or_else(|| {
                    err(".label không hợp lệ (một dòng, tối đa 120 ký tự)".to_string())
                })?,
        ),
    };

    let allowed_profiles = match obj.get("allowed_profiles").filter(|a| !a.is_null()) {
        None => None,
        Some(a) => {
            let bad = || {
                err(
                    ".allowed_profiles không hợp lệ (danh sách tên profile có trong profiles)"
                        .to_string(),
                )
            };
            let a = a.as_array().filter(|a| !a.is_empty()).ok_or_else(bad)?;
            let mut names = Vec::new();
            for n in a {
                match n.as_str() {
                    Some(s) if profile_names.iter().any(|p| p == s) => names.push(s.to_string()),
                    _ => return Err(bad()),
                }
            }
            Some(names)
        }
    };

    let pricing = match obj.get("pricing").filter(|p| !p.is_null()) {
        None => None,
        Some(_) if kind == EngineKind::Dsh => {
            return Err(err(
                ".pricing không dùng cho engine dsh (không có token)".to_string()
            ))
        }
        Some(p) => Some(parse_price(p, &label)?),
    };

    let spec = match kind {
        EngineKind::Anthropic => EngineSpec::Anthropic(parse_anthropic_entry(
            obj,
            &label,
            claude_bin,
            profile_names,
            allowed_profiles.as_deref(),
        )?),
        EngineKind::Agy => {
            let mut agy = AgyDef {
                command: DEFAULT_AGY_COMMAND.to_string(),
                print_timeout_margin_secs: DEFAULT_AGY_PRINT_TIMEOUT_MARGIN_SECS,
                model: None,
                dev_permission: DEFAULT_AGY_DEV_PERMISSION.to_string(),
            };
            apply_agy_fields(obj, &label, &mut agy)?;
            if let Some(p) = obj.get("dev_permission").filter(|p| !p.is_null()) {
                if p.as_str() != Some("accept-edits") {
                    return Err(err(
                        ".dev_permission không hợp lệ (chỉ accept-edits)".to_string()
                    ));
                }
            }
            EngineSpec::Agy(agy)
        }
        EngineKind::Dsh => EngineSpec::Dsh(parse_dsh_entry(obj, &label)?),
    };

    Ok(EngineDef {
        id,
        label: label_text,
        allowed_profiles,
        pricing,
        spec,
    })
}

fn parse_anthropic_entry(
    obj: &serde_json::Map<String, serde_json::Value>,
    label: &str,
    claude_bin: &str,
    profile_names: &[String],
    allowed_profiles: Option<&[String]>,
) -> BrainResult<AnthropicDef> {
    let err = |msg: String| BrainError::error(format!("Cấu hình {label}{msg}"));

    let command = match obj.get("command").filter(|c| !c.is_null()) {
        Some(c) => parse_command_field(c, label, "command")?,
        None => claude_bin.to_string(),
    };

    let base_url = match obj.get("base_url").filter(|u| !u.is_null()) {
        None => None,
        Some(u) => match u.as_str().map(str::trim) {
            Some(s) if s.starts_with("https://") => Some(s.to_string()),
            _ => return Err(err(".base_url phải bắt đầu bằng https://".to_string())),
        },
    };

    let key_env = match obj.get("key_env").filter(|k| !k.is_null()) {
        None => {
            if base_url.is_some() {
                return Err(err(".key_env bắt buộc khi có base_url".to_string()));
            }
            None
        }
        Some(k) => {
            if base_url.is_none() {
                return Err(err(".key_env chỉ dùng cùng base_url".to_string()));
            }
            match k.as_str() {
                Some(s) if valid_key_env(s) => Some(s.to_string()),
                _ => {
                    return Err(err(
                        ".key_env không hợp lệ (chỉ chữ hoa, số và _)".to_string()
                    ))
                }
            }
        }
    };

    let mut models = BTreeMap::new();
    if let Some(m) = obj.get("models").filter(|m| !m.is_null()) {
        let m = m
            .as_object()
            .ok_or_else(|| err(".models không hợp lệ (phải là JSON object)".to_string()))?;
        for (role, val) in m {
            match val.as_str().map(str::trim) {
                Some(s) if crate::teams::ROLES.contains(&role.as_str()) && !s.is_empty() => {
                    models.insert(role.clone(), s.to_string());
                }
                _ => return Err(err(format!(".models.{role} không hợp lệ"))),
            }
        }
    }
    if base_url.is_some() && !models.contains_key("dev") {
        return Err(err(".models.dev bắt buộc khi có base_url".to_string()));
    }

    let pin_profile = match obj.get("pin_profile").filter(|p| !p.is_null()) {
        None => None,
        Some(p) => {
            if base_url.is_some() {
                return Err(err(
                    ".pin_profile chỉ dùng cho engine không có base_url".to_string()
                ));
            }
            let name = p.as_str().unwrap_or("");
            if !profile_names.iter().any(|n| n == name) {
                return Err(err(format!(
                    ".pin_profile '{name}' không có trong profiles"
                )));
            }
            if let Some(allowed) = allowed_profiles {
                if !allowed.iter().any(|n| n == name) {
                    return Err(err(format!(
                        ".allowed_profiles phải chứa pin_profile '{name}'"
                    )));
                }
            }
            Some(name.to_string())
        }
    };

    let mut compat = Vec::new();
    if let Some(c) = obj.get("compat").filter(|c| !c.is_null()) {
        let bad = || {
            err(".compat không hợp lệ (chỉ disable_experimental_betas, disable_adaptive_thinking, cùng base_url)"
                .to_string())
        };
        if base_url.is_none() {
            return Err(bad());
        }
        for k in c.as_array().ok_or_else(bad)? {
            match k.as_str() {
                Some(s) if COMPAT_KNOBS.contains(&s) => {
                    if !compat.iter().any(|x: &String| x == s) {
                        compat.push(s.to_string());
                    }
                }
                _ => return Err(bad()),
            }
        }
    }

    let verified = match obj.get("verified").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => {
            if base_url.is_none() {
                return Err(err(".verified chỉ dùng cùng base_url".to_string()));
            }
            Some(parse_verified(v, label)?)
        }
    };

    Ok(AnthropicDef {
        command,
        base_url,
        key_env,
        models,
        pin_profile,
        compat,
        verified,
    })
}

fn parse_dsh_entry(
    obj: &serde_json::Map<String, serde_json::Value>,
    label: &str,
) -> BrainResult<DshDef> {
    let err = |msg: String| BrainError::error(format!("Cấu hình {label}{msg}"));

    let docker = match obj.get("docker").filter(|d| !d.is_null()) {
        Some(d) => parse_command_field(d, label, "docker")?,
        None => DEFAULT_DSH_DOCKER.to_string(),
    };

    let container = match obj.get("container").and_then(|c| c.as_str()).map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return Err(err(".container không được rỗng".to_string())),
    };
    let mut cs = container.chars();
    let first_ok = cs.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest_ok = cs.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if !first_ok || !rest_ok || container.len() > 64 {
        return Err(err(
            ".container không hợp lệ (chữ, số, _ . -; tối đa 64 ký tự)".to_string(),
        ));
    }

    let dsh_profile = match obj.get("dsh_profile").filter(|p| !p.is_null()) {
        None => DEFAULT_DSH_PROFILE.to_string(),
        Some(p) => match p.as_str() {
            Some(s) if crate::workflows::validate_name(s) => s.to_string(),
            _ => return Err(err(".dsh_profile không hợp lệ".to_string())),
        },
    };

    Ok(DshDef {
        docker,
        container,
        dsh_profile,
    })
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
    /// Optional `engines` block of config.json (D-08); absent means today's behaviour.
    pub engines: EnginesCfg,
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

    let profile_names: Vec<String> = profiles_val.keys().cloned().collect();
    let engines = parse_engines(obj.get("engines"), &claude_bin, &profile_names)?;

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
        engines,
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

    fn base_with(extra: serde_json::Value) -> serde_json::Value {
        let mut v: serde_json::Value = serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap();
        v.as_object_mut().unwrap().remove("engines");
        if let Some(o) = extra.as_object() {
            for (k, val) in o {
                v.as_object_mut().unwrap().insert(k.clone(), val.clone());
            }
        }
        v
    }

    fn deepseek_of(cfg: &Config) -> &AnthropicDef {
        cfg.engines
            .get("deepseek")
            .and_then(EngineDef::anthropic)
            .expect("deepseek entry")
    }

    fn agy_of(cfg: &Config) -> &AgyDef {
        cfg.engines
            .get("agy")
            .and_then(EngineDef::agy)
            .expect("agy entry")
    }

    #[test]
    fn engines_defaults() {
        // No `engines` key at all (as in Sandbox configs) behaves like the documented defaults.
        let cfg = config_from_value(&base_with(serde_json::json!({}))).unwrap();
        assert_eq!(cfg.engines.ids(), ["deepseek", "agy"]);
        let ds = deepseek_of(&cfg);
        assert_eq!(ds.command, cfg.claude_bin);
        assert_eq!(
            ds.base_url.as_deref(),
            Some("https://api.deepseek.com/anthropic")
        );
        assert_eq!(ds.key_env.as_deref(), Some("DEEPSEEK_API_KEY"));
        assert_eq!(ds.models, default_deepseek_models());
        assert_eq!(ds.models["lead"], "opus");
        assert_eq!(ds.models["dev"], "sonnet");
        assert_eq!(ds.models["qa"], "sonnet");
        assert_eq!(ds.models["plan-review"], "opus");
        assert_eq!(ds.models["code-review"], "opus");
        assert_eq!(ds.verified, None);
        assert_eq!(agy_of(&cfg).command, "agy");
        assert_eq!(agy_of(&cfg).print_timeout_margin_secs, 30);
        assert_eq!(agy_of(&cfg).model, None);
        assert_eq!(agy_of(&cfg).dev_permission, "accept-edits");

        // The default file documents the same values (the legacy object shape).
        let documented =
            config_from_value(&serde_json::from_str(DEFAULT_CONFIG_JSON).unwrap()).unwrap();
        assert_eq!(documented.engines, cfg.engines);

        // The deepseek command follows a custom claude_bin.
        let cfg = config_from_value(&base_with(serde_json::json!({"claude_bin": "/opt/claude"})))
            .unwrap();
        assert_eq!(deepseek_of(&cfg).command, "/opt/claude");

        // Explicit values; a partial models map keeps defaults for the missing roles.
        let cfg = config_from_value(&base_with(serde_json::json!({"engines": {
            "deepseek": {
                "command": "ds-claude",
                "base_url": "https://example.test/anthropic",
                "key_env": "MY_DS_KEY",
                "models": {"dev": "deepseek-chat"},
                "verified": "2026-10-28 E-09"
            },
            "agy": {"command": "/usr/local/bin/agy", "print_timeout_margin_secs": 45, "model": "gemini"}
        }})))
        .unwrap();
        let ds = deepseek_of(&cfg);
        assert_eq!(ds.command, "ds-claude");
        assert_eq!(
            ds.base_url.as_deref(),
            Some("https://example.test/anthropic")
        );
        assert_eq!(ds.key_env.as_deref(), Some("MY_DS_KEY"));
        assert_eq!(ds.models["dev"], "deepseek-chat");
        assert_eq!(ds.models["lead"], "opus");
        assert_eq!(ds.verified.as_deref(), Some("2026-10-28 E-09"));
        assert_eq!(agy_of(&cfg).command, "/usr/local/bin/agy");
        assert_eq!(agy_of(&cfg).print_timeout_margin_secs, 45);
        assert_eq!(agy_of(&cfg).model.as_deref(), Some("gemini"));
        // The legacy object is still exactly the two default ids.
        assert_eq!(cfg.engines.ids(), ["deepseek", "agy"]);
    }

    #[test]
    fn engines_validation() {
        let err = |engines: serde_json::Value| {
            config_from_value(&base_with(serde_json::json!({ "engines": engines })))
                .unwrap_err()
                .message
        };
        assert_eq!(
            err(serde_json::json!({"deepseek": {"key_env": "deepseek-key"}})),
            "Cấu hình engines.deepseek.key_env không hợp lệ (chỉ chữ hoa, số và _)"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"key_env": "1ABC"}})),
            "Cấu hình engines.deepseek.key_env không hợp lệ (chỉ chữ hoa, số và _)"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"base_url": "http://x"}})),
            "Cấu hình engines.deepseek.base_url phải bắt đầu bằng https://"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"command": ""}})),
            "Cấu hình engines.deepseek.command không được rỗng"
        );
        assert_eq!(
            err(serde_json::json!({"khac": {}})),
            "Cấu hình engines.khac không hợp lệ (chỉ deepseek, agy)"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"models": {"dev": ""}}})),
            "Cấu hình engines.deepseek.models.dev không hợp lệ"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"models": {"tester": "opus"}}})),
            "Cấu hình engines.deepseek.models.tester không hợp lệ"
        );
        assert_eq!(
            err(serde_json::json!({"agy": {"command": "  "}})),
            "Cấu hình engines.agy.command không được rỗng"
        );
        assert_eq!(
            err(serde_json::json!({"deepseek": {"verified": "dòng 1\ndòng 2"}})),
            "Cấu hình engines.deepseek.verified không hợp lệ (một dòng, tối đa 200 ký tự)"
        );
        // The only legacy text that changed: the top level may now be an array too.
        assert_eq!(
            err(serde_json::json!("x")),
            "Cấu hình engines không hợp lệ (phải là mảng hoặc JSON object)"
        );
        // `_` and digits after the first character are fine.
        assert!(config_from_value(&base_with(serde_json::json!({"engines": {
            "deepseek": {"key_env": "_DS_KEY_2"}
        }})))
        .is_ok());
    }

    fn gateway(id: &str, base: &str, key_env: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "kind": "anthropic", "base_url": base, "key_env": key_env,
            "models": {"lead": "deepseek/deepseek-reasoner", "dev": "deepseek/deepseek-chat", "qa": "deepseek/deepseek-chat"}
        })
    }

    #[test]
    fn engine_list() {
        let cfg = config_from_value(&base_with(serde_json::json!({"engines": [
            gateway("vcadm-deepseek", "https://gateway.invalid/proxyllm", "VCADM_GATEWAY_KEY"),
            {
                "id": "deepseek", "kind": "anthropic", "label": "DeepSeek chính thức",
                "base_url": "https://api.deepseek.com/anthropic", "key_env": "DEEPSEEK_API_KEY",
                "models": {"dev": "sonnet"}, "compat": ["disable_adaptive_thinking"],
                "verified": "2026-10-28 E-09", "pricing": {"input": 0.3, "output": 1.2, "cache_read": 0.03, "cache_write": 0.3}
            },
            {"id": "claude-max", "kind": "anthropic", "pin_profile": "personal", "models": {"lead": "opus"}, "_note": "ghi chú"},
            {"id": "agy", "kind": "agy"},
            {"id": "dsh", "kind": "dsh", "container": "dsh-deepseek-harness", "allowed_profiles": ["personal"]}
        ]})))
        .unwrap();

        assert_eq!(
            cfg.engines.ids(),
            ["vcadm-deepseek", "deepseek", "claude-max", "agy", "dsh"]
        );
        let kinds: Vec<&str> = cfg.engines.list.iter().map(|e| e.kind().as_str()).collect();
        assert_eq!(kinds, ["anthropic", "anthropic", "anthropic", "agy", "dsh"]);

        let gw = cfg.engines.get("vcadm-deepseek").unwrap();
        let a = gw.anthropic().unwrap();
        assert_eq!(a.command, cfg.claude_bin);
        assert_eq!(
            a.base_url.as_deref(),
            Some("https://gateway.invalid/proxyllm")
        );
        assert_eq!(gw.key_env(), Some("VCADM_GATEWAY_KEY"));
        assert!(!gw.is_native());
        assert_eq!(a.verified, None);
        assert_eq!(gw.effective_allowed_profiles(), None);

        let official = cfg.engines.get("deepseek").unwrap();
        assert_eq!(official.label.as_deref(), Some("DeepSeek chính thức"));
        assert_eq!(
            official.anthropic().unwrap().compat,
            ["disable_adaptive_thinking"]
        );
        assert_eq!(official.pricing.unwrap().output, 1.2);

        let max = cfg.engines.get("claude-max").unwrap();
        assert!(max.is_native());
        assert_eq!(max.key_env(), None);
        assert_eq!(
            max.effective_allowed_profiles(),
            Some(vec!["personal".to_string()])
        );

        let agy = cfg.engines.get("agy").unwrap().agy().unwrap();
        assert_eq!(agy.command, "agy");
        assert_eq!(agy.print_timeout_margin_secs, 30);
        assert_eq!(agy.dev_permission, "accept-edits");

        let dsh = cfg.engines.get("dsh").unwrap();
        let d = dsh.dsh().unwrap();
        assert_eq!(d.docker, "docker");
        assert_eq!(d.container, "dsh-deepseek-harness");
        assert_eq!(d.dsh_profile, "headless");
        assert_eq!(dsh.allowed_profiles, Some(vec!["personal".to_string()]));

        assert_eq!(
            cfg.engines.all_key_envs(),
            ["DEEPSEEK_API_KEY", "VCADM_GATEWAY_KEY"]
        );

        // Absent `engines` is the default catalog; a user array is used exactly as written.
        let none = config_from_value(&base_with(serde_json::json!({}))).unwrap();
        assert_eq!(none.engines, EnginesCfg::defaults(&none.claude_bin));
        let only_gw = config_from_value(&base_with(serde_json::json!({"engines": [
            gateway("gw", "https://gw.invalid/x", "GW_KEY")
        ]})))
        .unwrap();
        assert_eq!(only_gw.engines.ids(), ["gw"]);
        assert!(only_gw.engines.get("deepseek").is_none());
        assert!(only_gw.engines.get("agy").is_none());
        // An empty array is legal: claude and people only.
        let empty = config_from_value(&base_with(serde_json::json!({"engines": []}))).unwrap();
        assert!(empty.engines.list.is_empty());
        assert!(empty.engines.all_key_envs().is_empty());
    }

    #[test]
    fn kind_of_unknown_is_none() {
        let cfg = config_from_value(&base_with(serde_json::json!({}))).unwrap();
        assert_eq!(cfg.engines.kind_of("khong-co"), None);
        assert!(cfg.engines.get("khong-co").is_none());
        assert_eq!(cfg.engines.kind_of("deepseek"), Some(EngineKind::Anthropic));
        assert_eq!(cfg.engines.kind_of("agy"), Some(EngineKind::Agy));
        // `claude` is a built-in, never a list entry.
        assert_eq!(cfg.engines.kind_of("claude"), None);
    }

    /// The message of loading `engines` = `[entry]` (entry 0), or of `[prefix.., entry]`.
    fn entry_err(entry: serde_json::Value) -> String {
        config_from_value(&base_with(serde_json::json!({ "engines": [entry] })))
            .expect_err("entry must be refused")
            .message
    }

    fn engines_err(engines: serde_json::Value) -> String {
        config_from_value(&base_with(serde_json::json!({ "engines": engines })))
            .expect_err("engines must be refused")
            .message
    }

    #[test]
    fn engine_list_errors() {
        use serde_json::json;
        let gw = |extra: serde_json::Value| {
            let mut e = gateway("a", "https://a.invalid/x", "A_KEY");
            for (k, v) in extra.as_object().unwrap() {
                e[k] = v.clone();
            }
            e
        };

        // entry and top level
        assert_eq!(
            entry_err(json!(1)),
            "Cấu hình engines[0] không hợp lệ (phải là JSON object)"
        );
        assert_eq!(
            engines_err(json!(5)),
            "Cấu hình engines không hợp lệ (phải là mảng hoặc JSON object)"
        );

        // id
        let bad_id =
            "Cấu hình engines[0].id không hợp lệ (chữ thường không dấu, số và -, tối đa 32 ký tự)";
        assert_eq!(entry_err(json!({"kind": "agy"})), bad_id);
        assert_eq!(entry_err(json!({"id": "Bad Id", "kind": "agy"})), bad_id);
        assert_eq!(
            entry_err(json!({"id": "a".repeat(33), "kind": "agy"})),
            bad_id
        );
        assert_eq!(entry_err(json!({"id": 7, "kind": "agy"})), bad_id);
        assert_eq!(
            entry_err(json!({"id": "claude", "kind": "agy"})),
            "Cấu hình engines[0].id 'claude' là engine có sẵn — xóa mục này khỏi danh sách engines."
        );
        use unicode_normalization::UnicodeNormalization;
        let nfc = "người";
        let nfd: String = nfc.nfd().collect();
        assert_ne!(nfc, nfd, "test needs a genuinely different spelling");
        for spelling in [nfc.to_string(), nfd] {
            assert_eq!(
                engines_err(json!([{"id": "ok", "kind": "agy"}, {"id": spelling, "kind": "agy"}])),
                format!(
                    "Cấu hình engines[1].id '{spelling}' là engine có sẵn — xóa mục này khỏi danh sách engines."
                )
            );
        }
        assert_eq!(
            engines_err(
                json!([{"id": "a", "kind": "agy"}, {"id": "b", "kind": "agy"}, {"id": "a", "kind": "agy"}])
            ),
            "Cấu hình engines.a bị khai báo hai lần (engines[0] và engines[2])"
        );

        // kind
        let bad_kind = "Cấu hình engines.a.kind không hợp lệ (chỉ anthropic, agy, dsh)";
        assert_eq!(entry_err(json!({"id": "a", "kind": "openai"})), bad_kind);
        assert_eq!(entry_err(json!({"id": "a"})), bad_kind);

        // keys outside the kind's list; `_` keys are comments
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "env": {"A": "b"}})),
            "Cấu hình engines.a: khóa lạ env"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "anthropic", "docker": "docker"})),
            "Cấu hình engines.a: khóa lạ docker"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "dsh", "container": "c", "command": "x"})),
            "Cấu hình engines.a: khóa lạ command"
        );
        assert_eq!(
            entry_err(gw(json!({"args": ["--x"]}))),
            "Cấu hình engines.a: khóa lạ args"
        );
        assert!(config_from_value(&base_with(
            json!({"engines": [gw(json!({"_note": "ghi chú", "_x": {"y": 1}}))]})
        ))
        .is_ok());

        // label
        let bad_label = "Cấu hình engines.a.label không hợp lệ (một dòng, tối đa 120 ký tự)";
        assert_eq!(entry_err(gw(json!({"label": "a\nb"}))), bad_label);
        assert_eq!(entry_err(gw(json!({"label": "x".repeat(121)}))), bad_label);
        assert_eq!(entry_err(gw(json!({"label": 5}))), bad_label);
        assert!(config_from_value(&base_with(
            json!({"engines": [gw(json!({"label": "x".repeat(120)}))]})
        ))
        .is_ok());

        // allowed_profiles
        let bad_allowed =
            "Cấu hình engines.a.allowed_profiles không hợp lệ (danh sách tên profile có trong profiles)";
        for v in [
            json!([]),
            json!(["khong-co"]),
            json!("personal"),
            json!([1]),
        ] {
            assert_eq!(entry_err(gw(json!({"allowed_profiles": v}))), bad_allowed);
        }
        assert_eq!(
            entry_err(
                json!({"id": "a", "kind": "anthropic", "pin_profile": "personal", "allowed_profiles": ["work"]})
            ),
            "Cấu hình engines.a.allowed_profiles phải chứa pin_profile 'personal'"
        );

        // pricing
        let bad_price = "Cấu hình engines.a.pricing không hợp lệ (cần input, output, cache_read, cache_write là số không âm)";
        assert_eq!(entry_err(gw(json!({"pricing": {"input": 1}}))), bad_price);
        assert_eq!(
            entry_err(gw(
                json!({"pricing": {"input": -1, "output": 1, "cache_read": 1, "cache_write": 1}})
            )),
            bad_price
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "pricing": "x"})),
            bad_price
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "dsh", "container": "c",
                "pricing": {"input": 1, "output": 1, "cache_read": 1, "cache_write": 1}})),
            "Cấu hình engines.a.pricing không dùng cho engine dsh (không có token)"
        );

        // command (anthropic and agy) and docker (dsh): an executable only
        assert_eq!(
            entry_err(gw(json!({"command": ""}))),
            "Cấu hình engines.a.command không được rỗng"
        );
        assert_eq!(
            entry_err(gw(json!({"command": []}))),
            "Cấu hình engines.a.command không được rỗng"
        );
        assert_eq!(
            entry_err(gw(json!({"command": ["claude", "-p"]}))),
            "Cấu hình engines.a.command chỉ nhận tên lệnh, không nhận tham số"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "command": ["agy", "--x"]})),
            "Cấu hình engines.a.command chỉ nhận tên lệnh, không nhận tham số"
        );
        assert_eq!(
            entry_err(
                json!({"id": "a", "kind": "dsh", "container": "c", "docker": ["docker", "exec"]})
            ),
            "Cấu hình engines.a.docker chỉ nhận tên lệnh, không nhận tham số"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "dsh", "container": "c", "docker": " "})),
            "Cấu hình engines.a.docker không được rỗng"
        );
        let one = config_from_value(&base_with(
            json!({"engines": [gw(json!({"command": ["my-claude"]}))]}),
        ))
        .unwrap();
        assert_eq!(
            one.engines.get("a").unwrap().anthropic().unwrap().command,
            "my-claude"
        );

        // base_url / key_env
        assert_eq!(
            entry_err(gw(json!({"base_url": "http://x"}))),
            "Cấu hình engines.a.base_url phải bắt đầu bằng https://"
        );
        assert_eq!(
            entry_err(
                json!({"id": "a", "kind": "anthropic", "base_url": "https://x.invalid", "models": {"dev": "m"}})
            ),
            "Cấu hình engines.a.key_env bắt buộc khi có base_url"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "anthropic", "key_env": "A_KEY"})),
            "Cấu hình engines.a.key_env chỉ dùng cùng base_url"
        );
        for bad in ["a_key", "1KEY", "A-KEY", ""] {
            assert_eq!(
                entry_err(gw(json!({"key_env": bad}))),
                "Cấu hình engines.a.key_env không hợp lệ (chỉ chữ hoa, số và _)",
                "{bad:?}"
            );
        }

        // models
        assert_eq!(
            entry_err(gw(json!({"models": {"dev": "m", "tester": "x"}}))),
            "Cấu hình engines.a.models.tester không hợp lệ"
        );
        assert_eq!(
            entry_err(gw(json!({"models": {"dev": ""}}))),
            "Cấu hình engines.a.models.dev không hợp lệ"
        );
        assert_eq!(
            entry_err(gw(json!({"models": {"lead": "m"}}))),
            "Cấu hình engines.a.models.dev bắt buộc khi có base_url"
        );
        assert_eq!(
            entry_err(gw(json!({"models": ["m"]}))),
            "Cấu hình engines.a.models không hợp lệ (phải là JSON object)"
        );

        // pin_profile
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "anthropic", "pin_profile": "khong-co"})),
            "Cấu hình engines.a.pin_profile 'khong-co' không có trong profiles"
        );
        assert_eq!(
            entry_err(gw(json!({"pin_profile": "personal"}))),
            "Cấu hình engines.a.pin_profile chỉ dùng cho engine không có base_url"
        );

        // compat
        let bad_compat = "Cấu hình engines.a.compat không hợp lệ (chỉ disable_experimental_betas, disable_adaptive_thinking, cùng base_url)";
        assert_eq!(entry_err(gw(json!({"compat": ["bat-ky"]}))), bad_compat);
        assert_eq!(entry_err(gw(json!({"compat": "x"}))), bad_compat);
        assert_eq!(
            entry_err(
                json!({"id": "a", "kind": "anthropic", "compat": ["disable_experimental_betas"]})
            ),
            bad_compat
        );

        // verified
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "anthropic", "verified": "x"})),
            "Cấu hình engines.a.verified chỉ dùng cùng base_url"
        );
        let bad_verified = "Cấu hình engines.a.verified không hợp lệ (một dòng, tối đa 200 ký tự)";
        assert_eq!(entry_err(gw(json!({"verified": "x\ny"}))), bad_verified);
        assert_eq!(
            entry_err(gw(json!({"verified": "x".repeat(201)}))),
            bad_verified
        );

        // agy fields (the legacy texts with `engines.{id}.`)
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "print_timeout_margin_secs": 3601})),
            "Cấu hình engines.a.print_timeout_margin_secs phải là số nguyên từ 0 đến 3600"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "model": " "})),
            "Cấu hình engines.a.model không hợp lệ"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "agy", "dev_permission": "skip"})),
            "Cấu hình engines.a.dev_permission không hợp lệ (chỉ accept-edits)"
        );

        // dsh fields
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "dsh"})),
            "Cấu hình engines.a.container không được rỗng"
        );
        assert_eq!(
            entry_err(json!({"id": "a", "kind": "dsh", "container": ""})),
            "Cấu hình engines.a.container không được rỗng"
        );
        let bad_container =
            "Cấu hình engines.a.container không hợp lệ (chữ, số, _ . -; tối đa 64 ký tự)";
        for bad in ["-x", "a b", "a/b", &"c".repeat(65)] {
            assert_eq!(
                entry_err(json!({"id": "a", "kind": "dsh", "container": bad})),
                bad_container,
                "{bad:?}"
            );
        }
        assert_eq!(
            entry_err(
                json!({"id": "a", "kind": "dsh", "container": "c", "dsh_profile": "Bad Name"})
            ),
            "Cấu hình engines.a.dsh_profile không hợp lệ"
        );
        let ok = config_from_value(&base_with(
            json!({"engines": [{"id": "a", "kind": "dsh", "container": "Dsh_1.x-y"}]}),
        ))
        .unwrap();
        assert_eq!(
            ok.engines.get("a").unwrap().dsh().unwrap().container,
            "Dsh_1.x-y"
        );
    }

    #[test]
    fn example_engines_file_loads() {
        let text = include_str!("../../docs/config.engines.example.json");
        let example: serde_json::Value = serde_json::from_str(text).expect("example is JSON");
        let engines = example["engines"].clone();
        assert!(engines.is_array(), "the example has an `engines` array");

        // Only `engines` is merged, so `profiles` stays the documented default, which declares
        // the `personal` profile the example's `pin_profile` and `allowed_profiles` name.
        let merged = base_with(serde_json::json!({ "engines": engines }));
        assert!(
            merged["profiles"].get("personal").is_some(),
            "the default profiles must declare `personal`"
        );
        let cfg = config_from_value(&merged).expect("the shipped example must load");

        assert_eq!(
            cfg.engines.ids(),
            ["deepseek", "vcadm-deepseek", "claude-max", "agy", "dsh"]
        );
        assert!(cfg.engines.get("claude").is_none());
        let max = cfg.engines.get("claude-max").unwrap();
        assert!(max.is_native());
        assert_eq!(
            max.effective_allowed_profiles(),
            Some(vec!["personal".to_string()])
        );
        let dsh = cfg.engines.get("dsh").unwrap();
        assert_eq!(dsh.allowed_profiles, Some(vec!["personal".to_string()]));
        assert_eq!(dsh.dsh().unwrap().container, "dsh-deepseek-harness");
        assert_eq!(
            cfg.engines.all_key_envs(),
            ["DEEPSEEK_API_KEY", "VCADM_GATEWAY_KEY"]
        );
        // Key NAMES only: no entry carries an env map or a command array.
        for e in engines_array(&example) {
            assert!(e.get("env").is_none(), "{e}");
            assert!(!e["command"].is_array(), "{e}");
        }
    }

    fn engines_array(example: &serde_json::Value) -> Vec<&serde_json::Value> {
        example["engines"].as_array().unwrap().iter().collect()
    }
}
