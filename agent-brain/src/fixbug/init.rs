//! Khởi tạo file cấu hình .agent-brain/fixbugs.json cho dự án (Plan fixbug-sources-engine).
//!
//! Tự động đọc cấu hình cũ từ .gsd/fix-bugs/config.json (nếu có) để giữ lại
//! comment_prefix, tên list và labels; hoặc sinh cấu hình mẫu mặc định.
//! Hỗ trợ --dry-run; chỉ ghi khi file chưa tồn tại (không bao giờ ghi đè).

use super::config::{config_path, FixbugConfig, PriorityLabels, SourceConfig, TrelloConfig};
use super::envfile::load_env;
use super::trello::{trello_api_base, TrelloClient};
use super::FixbugError;
use std::path::Path;

/// Khởi tạo file cấu hình fixbugs.json cho dự án.
/// Trả về nội dung JSON đề xuất (và danh sách list trên board nếu đọc được).
pub fn init(project_root: &Path, dry_run: bool) -> Result<String, FixbugError> {
    let out_path = config_path(project_root);
    if !dry_run && out_path.is_file() {
        return Err(FixbugError::Config(format!(
            "File cấu hình {} đã tồn tại, không ghi đè — hãy xoá hoặc sửa file trực tiếp",
            out_path.display()
        )));
    }

    let gsd_config_path = project_root.join(".gsd/fix-bugs/config.json");
    let mut trello_cfg = TrelloConfig::default();

    if gsd_config_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&gsd_config_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(trello) = val.get("trello") {
                    if let Some(intake) = trello.get("lists").and_then(|l| l.get("intake")).and_then(|i| i.as_array()) {
                        let lists: Vec<String> = intake
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !lists.is_empty() {
                            trello_cfg.intake_lists = lists;
                        }
                    }
                    if let Some(fixed) = trello.get("lists").and_then(|l| l.get("fixed")).and_then(|s| s.as_str()) {
                        trello_cfg.fixed_list = fixed.to_string();
                    }
                    if let Some(rejected) = trello.get("lists").and_then(|l| l.get("rejected")).and_then(|s| s.as_str()) {
                        trello_cfg.rejected_list = rejected.to_string();
                    }
                    if let Some(labels) = trello.get("labels").and_then(|l| l.as_array()) {
                        trello_cfg.labels = labels
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                    if let Some(prefix) = trello.get("comment_prefix").and_then(|s| s.as_str()) {
                        trello_cfg.comment_prefix = prefix.to_string();
                    }
                }
            }
        }
    }

    trello_cfg.priority_labels = PriorityLabels {
        urgent: vec!["high".to_string()],
        low: Vec::new(),
    };

    let full_config = FixbugConfig {
        sources: vec![SourceConfig::Trello(trello_cfg)],
    };

    let json_text = serde_json::to_string_pretty(&full_config)
        .map_err(|e| FixbugError::Config(e.to_string()))?;

    // Thử kết nối Trello để liệt kê danh sách list thật nếu có credentials
    let secrets = load_env(project_root);
    let mut board_info = String::new();
    if let (Some(key), Some(token), Some(board_id)) = (
        secrets.trello_api_key(),
        secrets.trello_token(),
        secrets.trello_board_id(),
    ) {
        if !key.is_empty() && !token.is_empty() && !board_id.is_empty() {
            if let Ok(base) = trello_api_base() {
                let client = TrelloClient::new(&base, key, token);
                match client.lists(board_id) {
                    Ok(lists) => {
                        let names: Vec<_> = lists.iter().map(|l| format!("'{}'", l.name)).collect();
                        board_info = format!("\n# Các list hiện có trên board Trello {board_id}: [{}]", names.join(", "));
                    }
                    Err(e) => {
                        board_info = format!("\n# Cảnh báo: không truy vấn được list trên Trello ({e})");
                    }
                }
            }
        }
    }

    if !dry_run {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                FixbugError::Io(format!("Không tạo được thư mục {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&out_path, format!("{json_text}\n")).map_err(|e| {
            FixbugError::Io(format!("Không ghi được file {}: {e}", out_path.display()))
        })?;
    }

    if board_info.is_empty() {
        Ok(json_text)
    } else {
        Ok(format!("{json_text}{board_info}"))
    }
}
