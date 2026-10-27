//! Danh mục engine cố định (D-08): claude, deepseek, agy, người.
//! Lệnh, endpoint và biến key chỉ nằm trong config.json; ở đây chỉ có kiểu và độ sẵn sàng tính tại máy.

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

#[cfg(test)]
mod tests {
    use super::*;

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
