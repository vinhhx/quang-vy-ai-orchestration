//! Parser tối thiểu cho <root>/.env.fixbugs (chứa bí mật Trello).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const ENV_FIXBUGS_FILENAME: &str = ".env.fixbugs";

/// Lưu trữ các biến môi trường từ .env.fixbugs.
/// `Debug` luôn in `<ẩn>` để không lộ bí mật khi log hoặc format.
#[derive(Clone, PartialEq, Eq)]
pub struct FixbugSecrets {
    map: BTreeMap<String, String>,
}

impl std::fmt::Debug for FixbugSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FixbugSecrets(<ẩn>)")
    }
}

impl FixbugSecrets {
    pub fn new(map: BTreeMap<String, String>) -> Self {
        Self { map }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|s| s.as_str())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn trello_api_key(&self) -> Option<&str> {
        self.get("TRELLO_API_KEY")
    }

    pub fn trello_token(&self) -> Option<&str> {
        self.get("TRELLO_TOKEN")
    }

    pub fn trello_board_id(&self) -> Option<&str> {
        self.get("TRELLO_BOARD_ID")
    }

    /// Trả về danh sách chuỗi bí mật (key, token) để redact khỏi logs/lỗi.
    pub fn redaction_secrets(&self) -> Vec<String> {
        let mut list = Vec::new();
        if let Some(k) = self.trello_api_key() {
            if !k.trim().is_empty() {
                list.push(k.trim().to_string());
            }
        }
        if let Some(t) = self.trello_token() {
            if !t.trim().is_empty() {
                list.push(t.trim().to_string());
            }
        }
        list
    }
}

pub fn env_path(project_root: &Path) -> PathBuf {
    project_root.join(ENV_FIXBUGS_FILENAME)
}

/// Đọc và phân tích file `.env.fixbugs` trong thư mục gốc dự án.
/// Nếu file không tồn tại, trả về `FixbugSecrets` rỗng (không báo lỗi).
pub fn load_env(project_root: &Path) -> FixbugSecrets {
    let path = env_path(project_root);
    if !path.is_file() {
        return FixbugSecrets::new(BTreeMap::new());
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return FixbugSecrets::new(BTreeMap::new());
    };

    parse_env_content(&content)
}

/// Phân tích nội dung text theo quy tắc .env tối thiểu:
/// - Dòng KEY=VALUE
/// - Bỏ dòng trống và dòng bắt đầu bằng '#'
/// - Bỏ tiền tố 'export '
/// - Bỏ một cặp nháy bao ngoài ('...' hoặc "...")
pub fn parse_env_content(content: &str) -> FixbugSecrets {
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let without_export = if let Some(stripped) = trimmed.strip_prefix("export ") {
            stripped.trim()
        } else {
            trimmed
        };

        if let Some((key, val)) = without_export.split_once('=') {
            let k = key.trim();
            if k.is_empty() {
                continue;
            }
            let mut v = val.trim();
            if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
                || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
            {
                v = &v[1..v.len() - 1];
            }
            map.insert(k.to_string(), v.to_string());
        }
    }
    FixbugSecrets::new(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_lines() {
        let content = r#"
        # Đây là ghi chú
        TRELLO_API_KEY=key_12345
        TRELLO_TOKEN="token_67890"
        export TRELLO_BOARD_ID='board_abc'
        EMPTY=
        OTHER=value with spaces
        "#;

        let secrets = parse_env_content(content);
        assert_eq!(secrets.trello_api_key(), Some("key_12345"));
        assert_eq!(secrets.trello_token(), Some("token_67890"));
        assert_eq!(secrets.trello_board_id(), Some("board_abc"));
        assert_eq!(secrets.get("EMPTY"), Some(""));
        assert_eq!(secrets.get("OTHER"), Some("value with spaces"));
        assert_eq!(secrets.get("NOT_FOUND"), None);
    }

    #[test]
    fn debug_masks_secrets() {
        let mut map = BTreeMap::new();
        map.insert("TRELLO_API_KEY".to_string(), "super_secret".to_string());
        let secrets = FixbugSecrets::new(map);
        let debug_str = format!("{secrets:?}");
        assert_eq!(debug_str, "FixbugSecrets(<ẩn>)");
        assert!(!debug_str.contains("super_secret"));
    }
}
