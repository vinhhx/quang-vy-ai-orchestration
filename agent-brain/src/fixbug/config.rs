//! Đọc và kiểm tra cấu hình fixbug theo dự án (.agent-brain/fixbugs.json).

use super::FixbugError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub const CONFIG_REL_PATH: &str = ".agent-brain/fixbugs.json";

/// Chuẩn hóa chuỗi để so khớp tên cột, list, nhãn, giá trị (trim, NFC, chữ thường).
pub fn normalize_match(s: &str) -> String {
    s.trim().nfc().collect::<String>().to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityLabels {
    #[serde(default = "default_urgent_labels")]
    pub urgent: Vec<String>,
    #[serde(default)]
    pub low: Vec<String>,
}

fn default_urgent_labels() -> Vec<String> {
    vec!["high".to_string()]
}

impl Default for PriorityLabels {
    fn default() -> Self {
        Self {
            urgent: default_urgent_labels(),
            low: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrelloConfig {
    #[serde(default)]
    pub board: Option<String>,
    #[serde(default = "default_intake_lists")]
    pub intake_lists: Vec<String>,
    #[serde(default = "default_fixed_list")]
    pub fixed_list: String,
    #[serde(default = "default_rejected_list")]
    pub rejected_list: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub priority_labels: PriorityLabels,
    #[serde(default = "default_comment_prefix")]
    pub comment_prefix: String,
}

fn default_intake_lists() -> Vec<String> {
    vec!["Bugs".to_string()]
}

fn default_fixed_list() -> String {
    "Testing".to_string()
}

fn default_rejected_list() -> String {
    "Reject".to_string()
}

fn default_comment_prefix() -> String {
    "[agent-brain]".to_string()
}

impl Default for TrelloConfig {
    fn default() -> Self {
        Self {
            board: None,
            intake_lists: default_intake_lists(),
            fixed_list: default_fixed_list(),
            rejected_list: default_rejected_list(),
            labels: Vec::new(),
            priority_labels: PriorityLabels::default(),
            comment_prefix: default_comment_prefix(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GsheetColumns {
    #[serde(default = "default_col_id")]
    pub id: String,
    #[serde(default = "default_col_group")]
    pub group: Option<String>,
    #[serde(default = "default_col_content")]
    pub content: String,
    #[serde(default = "default_col_status")]
    pub status: String,
    #[serde(default = "default_col_priority")]
    pub priority: Option<String>,
    #[serde(default = "default_col_attachment")]
    pub attachment: Option<String>,
    #[serde(default = "default_col_reporter")]
    pub reporter: Option<String>,
}

fn default_col_id() -> String {
    "STT".to_string()
}

fn default_col_group() -> Option<String> {
    Some("Hạng mục".to_string())
}

fn default_col_content() -> String {
    "Nội dung phản hồi".to_string()
}

fn default_col_status() -> String {
    "Trạng thái".to_string()
}

fn default_col_priority() -> Option<String> {
    Some("Mức độ Ưu tiên".to_string())
}

fn default_col_attachment() -> Option<String> {
    Some("Mô tả/chụp màn hình".to_string())
}

fn default_col_reporter() -> Option<String> {
    Some("Nhân sự feedback".to_string())
}

impl Default for GsheetColumns {
    fn default() -> Self {
        Self {
            id: default_col_id(),
            group: default_col_group(),
            content: default_col_content(),
            status: default_col_status(),
            priority: default_col_priority(),
            attachment: default_col_attachment(),
            reporter: default_col_reporter(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityValues {
    #[serde(default = "default_urgent_values")]
    pub urgent: Vec<String>,
    #[serde(default = "default_low_values")]
    pub low: Vec<String>,
}

fn default_urgent_values() -> Vec<String> {
    vec!["Cao".to_string(), "High".to_string(), "Gấp".to_string()]
}

fn default_low_values() -> Vec<String> {
    vec!["Thấp".to_string(), "Low".to_string()]
}

impl Default for PriorityValues {
    fn default() -> Self {
        Self {
            urgent: default_urgent_values(),
            low: default_low_values(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GsheetConfig {
    pub url: String,
    #[serde(default)]
    pub columns: GsheetColumns,
    #[serde(default = "default_todo_status")]
    pub todo_status: Vec<String>,
    #[serde(default)]
    pub priority_values: PriorityValues,
}

fn default_todo_status() -> Vec<String> {
    vec!["".to_string()]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SourceConfig {
    #[serde(rename = "trello")]
    Trello(TrelloConfig),
    #[serde(rename = "gsheet")]
    Gsheet(GsheetConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FixbugConfig {
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
}

/// Trích xuất `(spreadsheet_id, gid)` từ URL Google Sheet.
/// Chấp nhận các dạng /edit…, /edit#gid=…, /edit?gid=…, /htmlview…, hoặc link export?format=csv.
/// Host phải là docs.google.com. Thiếu gid → "0".
pub fn parse_gsheet_url(raw_url: &str) -> Result<(String, String), FixbugError> {
    let s = raw_url.trim();
    let without_scheme = if let Some(r) = s.strip_prefix("https://") {
        r
    } else if let Some(r) = s.strip_prefix("http://") {
        r
    } else {
        return Err(FixbugError::Config(format!(
            "URL Google Sheet không hợp lệ: '{raw_url}'"
        )));
    };

    let (host, path_and_query) = match without_scheme.split_once('/') {
        Some((h, p)) => (h, p),
        None => (without_scheme, ""),
    };

    let host_clean = host.split(':').next().unwrap_or(host);
    if host_clean != "docs.google.com" {
        return Err(FixbugError::Config(format!(
            "URL Google Sheet phải có host docs.google.com (nhận '{host_clean}')"
        )));
    }

    // Path dạng spreadsheets/d/<id>/...
    let id_marker = "spreadsheets/d/";
    let Some(id_start) = path_and_query.find(id_marker) else {
        return Err(FixbugError::Config(format!(
            "URL Google Sheet thiếu đường dẫn /spreadsheets/d/<id>: '{raw_url}'"
        )));
    };

    let after_marker = &path_and_query[id_start + id_marker.len()..];
    let sheet_id = match after_marker.find('/') {
        Some(idx) => &after_marker[..idx],
        None => match after_marker.find(['?', '#']) {
            Some(idx) => &after_marker[..idx],
            None => after_marker,
        },
    };

    if sheet_id.is_empty() {
        return Err(FixbugError::Config(format!(
            "Không tìm thấy id của bảng tính trong URL Google Sheet: '{raw_url}'"
        )));
    }

    // Trích xuất gid từ query (?gid=... hoặc &gid=...) hoặc fragment (#gid=... hoặc &gid=...)
    let gid = extract_gid(path_and_query).unwrap_or_else(|| "0".to_string());

    Ok((sheet_id.to_string(), gid))
}

fn extract_gid(path_and_query: &str) -> Option<String> {
    // Tìm trong query hoặc fragment
    for part in path_and_query.split(['?', '#', '&']) {
        if let Some(val) = part.strip_prefix("gid=") {
            let clean = val.split(['&', '#', ';', '/']).next().unwrap_or(val).trim();
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }
    None
}

pub fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".agent-brain").join("fixbugs.json")
}

pub fn load_config(project_root: &Path) -> Result<FixbugConfig, FixbugError> {
    let path = config_path(project_root);
    if !path.is_file() {
        return Err(FixbugError::NotConfigured(format!(
            "Dự án chưa có file cấu hình {} — chạy 'agent-brain fixbug init' để khởi tạo",
            path.display()
        )));
    }

    let bytes = std::fs::read(&path).map_err(|e| {
        FixbugError::Io(format!("Không đọc được {}: {e}", path.display()))
    })?;

    let text = String::from_utf8(bytes).map_err(|e| {
        FixbugError::Config(format!("File {} không phải UTF-8 hợp lệ: {e}", path.display()))
    })?;

    let cfg: FixbugConfig = serde_json::from_str(&text).map_err(|e| {
        FixbugError::Config(format!("Cấu hình {}: {e}", path.display()))
    })?;

    for (idx, src) in cfg.sources.iter().enumerate() {
        if let SourceConfig::Gsheet(g) = src {
            if let Err(e) = parse_gsheet_url(&g.url) {
                return Err(FixbugError::Config(format!(
                    "Cấu hình {}: nguồn gsheet #{idx}: {e}",
                    path.display()
                )));
            }
        }
    }

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_example_config() {
        let json = r#"{
          "sources": [
            { "type": "trello",
              "board": null,
              "intake_lists": ["Bugs"],
              "fixed_list": "Testing",
              "rejected_list": "Reject",
              "labels": [],
              "priority_labels": { "urgent": ["high"], "low": [] },
              "comment_prefix": "[agent-brain]" },
            { "type": "gsheet",
              "url": "https://docs.google.com/spreadsheets/d/abc-123/edit?gid=456#gid=456",
              "columns": { "id": "STT", "group": "Hạng mục", "content": "Nội dung phản hồi",
                           "status": "Trạng thái", "priority": "Mức độ Ưu tiên",
                           "attachment": "Mô tả/chụp màn hình", "reporter": "Nhân sự feedback" },
              "todo_status": [""],
              "priority_values": { "urgent": ["Cao", "High", "Gấp"], "low": ["Thấp", "Low"] } }
          ]
        }"#;

        let cfg: FixbugConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.sources.len(), 2);
        match &cfg.sources[0] {
            SourceConfig::Trello(t) => {
                assert_eq!(t.board, None);
                assert_eq!(t.intake_lists, vec!["Bugs"]);
                assert_eq!(t.fixed_list, "Testing");
                assert_eq!(t.rejected_list, "Reject");
                assert_eq!(t.comment_prefix, "[agent-brain]");
                assert_eq!(t.priority_labels.urgent, vec!["high"]);
            }
            _ => panic!("Expected Trello"),
        }
        match &cfg.sources[1] {
            SourceConfig::Gsheet(g) => {
                assert_eq!(
                    g.url,
                    "https://docs.google.com/spreadsheets/d/abc-123/edit?gid=456#gid=456"
                );
                assert_eq!(g.columns.id, "STT");
                assert_eq!(g.columns.content, "Nội dung phản hồi");
                assert_eq!(g.todo_status, vec![""]);
                assert_eq!(g.priority_values.urgent, vec!["Cao", "High", "Gấp"]);
            }
            _ => panic!("Expected Gsheet"),
        }
    }

    #[test]
    fn deny_unknown_fields_in_config() {
        let json = r#"{
          "sources": [
            { "type": "trello", "unknown_field": 123 }
          ]
        }"#;
        let res: Result<FixbugConfig, _> = serde_json::from_str(json);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("unknown field `unknown_field`"));
    }

    #[test]
    fn parse_gsheet_urls() {
        let (id, gid) = parse_gsheet_url(
            "https://docs.google.com/spreadsheets/d/1iLTQ1Q560p38TNSpY-tvgUP2m1Upgfj_U1SHzY1dVU4/edit?gid=415319006#gid=415319006",
        )
        .unwrap();
        assert_eq!(id, "1iLTQ1Q560p38TNSpY-tvgUP2m1Upgfj_U1SHzY1dVU4");
        assert_eq!(gid, "415319006");

        let (id, gid) = parse_gsheet_url(
            "https://docs.google.com/spreadsheets/d/my-sheet-id/edit#gid=12345",
        )
        .unwrap();
        assert_eq!(id, "my-sheet-id");
        assert_eq!(gid, "12345");

        let (id, gid) = parse_gsheet_url(
            "https://docs.google.com/spreadsheets/d/my-sheet-id/htmlview",
        )
        .unwrap();
        assert_eq!(id, "my-sheet-id");
        assert_eq!(gid, "0");

        let (id, gid) = parse_gsheet_url(
            "https://docs.google.com/spreadsheets/d/my-sheet-id/export?format=csv&gid=789",
        )
        .unwrap();
        assert_eq!(id, "my-sheet-id");
        assert_eq!(gid, "789");

        // Wrong host
        let err = parse_gsheet_url("https://other.domain.com/spreadsheets/d/id/edit").unwrap_err();
        assert!(err.to_string().contains("docs.google.com"));
    }

    #[test]
    fn test_normalize_match() {
        assert_eq!(normalize_match("  Bugs  "), "bugs");
        assert_eq!(normalize_match("Cao"), "cao");
        assert_eq!(normalize_match("Hoàn Thành"), "hoàn thành");
    }
}
