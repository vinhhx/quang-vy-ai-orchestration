//! Ghi ngược trạng thái sang Trello (Plan fixbug-sources-engine).
//!
//! Chỉ chạy khi người dùng xác nhận:
//! - `deployed`: chuyển card Trello sang list Testing, thêm comment prefix + note, đổi state ref thành 'deployed'.
//! - `reject`: chuyển card Trello sang list Reject, comment lý do, đổi state ref thành 'rejected'.
//! - Đối với bug Google Sheet (key S-…): chỉ đổi state trong CSDL và trả về lời nhắc cập nhật thủ công.

use super::config::{load_config, normalize_match, SourceConfig};
use super::envfile::load_env;
use super::trello::{trello_api_base, TrelloClient};
use super::FixbugError;
use crate::db::{fixbug_ref_by_key, set_fixbug_ref_state};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WritebackItemResult {
    pub key: String,
    pub state: String,
    pub message: String,
}

/// Đánh dấu các bug đã sửa và deploy.
pub fn deployed(
    conn: &mut Connection,
    project_root: &Path,
    project_id: i64,
    keys: &[String],
    note: Option<&str>,
) -> Result<Vec<WritebackItemResult>, FixbugError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let config = load_config(project_root)?;
    let secrets = load_env(project_root);
    let mut results = Vec::new();

    for key in keys {
        let clean_key = key.trim();
        let ref_row = fixbug_ref_by_key(conn, project_id, clean_key)
            .map_err(|e| FixbugError::Other(e.to_string()))?
            .ok_or_else(|| FixbugError::Other(format!("Không tìm thấy ref '{clean_key}' trong dự án")))?;

        if ref_row.state != "pulled" {
            return Err(FixbugError::Other(format!(
                "Ref '{clean_key}' đang ở trạng thái '{}', chỉ có thể deploy khi ở trạng thái 'pulled'",
                ref_row.state
            )));
        }

        if clean_key.starts_with("T-") {
            let t_cfg = config.sources.iter().find_map(|s| match s {
                SourceConfig::Trello(t) => Some(t),
                _ => None,
            }).ok_or_else(|| FixbugError::Config("Dự án chưa cấu hình nguồn Trello".to_string()))?;

            let board_id = t_cfg
                .board
                .as_deref()
                .or_else(|| secrets.trello_board_id())
                .unwrap_or("");
            if board_id.is_empty() {
                return Err(FixbugError::Config("Thiếu board id trong cấu hình Trello".to_string()));
            }

            let trello_key = secrets.trello_api_key().unwrap_or("");
            let trello_token = secrets.trello_token().unwrap_or("");
            if trello_key.is_empty() || trello_token.is_empty() {
                return Err(FixbugError::Config("Thiếu TRELLO_API_KEY hoặc TRELLO_TOKEN trong .env.fixbugs".to_string()));
            }

            let base = trello_api_base()?;
            let client = TrelloClient::new(&base, trello_key, trello_token);

            let all_lists = client.lists(board_id)?;
            let norm_dest = normalize_match(&t_cfg.fixed_list);
            let dest_list = all_lists.iter().find(|l| normalize_match(&l.name) == norm_dest)
                .ok_or_else(|| FixbugError::Config(format!("List fixed '{}' không tồn tại trên board Trello", t_cfg.fixed_list)))?;

            // Kiểm tra list hiện tại của card
            let (cur_list_id, cur_list_name) = client.card_list_name(&ref_row.ext_id, Some(board_id))?;
            let already_in_dest = cur_list_id == dest_list.id || normalize_match(&cur_list_name) == norm_dest;

            if already_in_dest {
                set_fixbug_ref_state(conn, ref_row.id, "deployed", note)
                    .map_err(|e| FixbugError::Other(e.to_string()))?;
                results.push(WritebackItemResult {
                    key: clean_key.to_string(),
                    state: "deployed".to_string(),
                    message: format!("Thẻ đã nằm trong list '{}', chỉ cập nhật trạng thái", dest_list.name),
                });
            } else {
                let comment_text = match note {
                    Some(n) if !n.trim().is_empty() => format!("Đã sửa và deploy. {}", n.trim()),
                    _ => "Đã sửa và deploy.".to_string(),
                };
                client.add_comment(&ref_row.ext_id, &comment_text, &t_cfg.comment_prefix)?;
                client.move_card(&ref_row.ext_id, &dest_list.id)?;
                set_fixbug_ref_state(conn, ref_row.id, "deployed", note)
                    .map_err(|e| FixbugError::Other(e.to_string()))?;
                results.push(WritebackItemResult {
                    key: clean_key.to_string(),
                    state: "deployed".to_string(),
                    message: format!("Đã chuyển thẻ sang list '{}' và thêm comment", dest_list.name),
                });
            }
        } else if clean_key.starts_with("S-") {
            set_fixbug_ref_state(conn, ref_row.id, "deployed", note)
                .map_err(|e| FixbugError::Other(e.to_string()))?;
            results.push(WritebackItemResult {
                key: clean_key.to_string(),
                state: "deployed".to_string(),
                message: "Đã cập nhật trạng thái local thành 'deployed'. Nhắc: hãy cập nhật cột Trạng thái trên Google Sheet thủ công.".to_string(),
            });
        } else {
            return Err(FixbugError::Other(format!(
                "Key '{clean_key}' không hợp lệ (phải bắt đầu bằng T- hoặc S-)"
            )));
        }
    }

    Ok(results)
}

/// Từ chối sửa bug.
pub fn reject(
    conn: &mut Connection,
    project_root: &Path,
    project_id: i64,
    keys: &[String],
    reason: &str,
) -> Result<Vec<WritebackItemResult>, FixbugError> {
    let clean_reason = reason.trim();
    if clean_reason.is_empty() {
        return Err(FixbugError::Other(
            "Lý do từ chối sửa (--reason) là bắt buộc".to_string(),
        ));
    }

    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let config = load_config(project_root)?;
    let secrets = load_env(project_root);
    let mut results = Vec::new();

    for key in keys {
        let clean_key = key.trim();
        let ref_row = fixbug_ref_by_key(conn, project_id, clean_key)
            .map_err(|e| FixbugError::Other(e.to_string()))?
            .ok_or_else(|| FixbugError::Other(format!("Không tìm thấy ref '{clean_key}' trong dự án")))?;

        if ref_row.state != "pulled" {
            return Err(FixbugError::Other(format!(
                "Ref '{clean_key}' đang ở trạng thái '{}', chỉ có thể reject khi ở trạng thái 'pulled'",
                ref_row.state
            )));
        }

        if clean_key.starts_with("T-") {
            let t_cfg = config.sources.iter().find_map(|s| match s {
                SourceConfig::Trello(t) => Some(t),
                _ => None,
            }).ok_or_else(|| FixbugError::Config("Dự án chưa cấu hình nguồn Trello".to_string()))?;

            let board_id = t_cfg
                .board
                .as_deref()
                .or_else(|| secrets.trello_board_id())
                .unwrap_or("");
            if board_id.is_empty() {
                return Err(FixbugError::Config("Thiếu board id trong cấu hình Trello".to_string()));
            }

            let trello_key = secrets.trello_api_key().unwrap_or("");
            let trello_token = secrets.trello_token().unwrap_or("");
            if trello_key.is_empty() || trello_token.is_empty() {
                return Err(FixbugError::Config("Thiếu TRELLO_API_KEY hoặc TRELLO_TOKEN trong .env.fixbugs".to_string()));
            }

            let base = trello_api_base()?;
            let client = TrelloClient::new(&base, trello_key, trello_token);

            let all_lists = client.lists(board_id)?;
            let norm_dest = normalize_match(&t_cfg.rejected_list);
            let dest_list = all_lists.iter().find(|l| normalize_match(&l.name) == norm_dest)
                .ok_or_else(|| FixbugError::Config(format!("List rejected '{}' không tồn tại trên board Trello", t_cfg.rejected_list)))?;

            // Kiểm tra list hiện tại của card
            let (cur_list_id, cur_list_name) = client.card_list_name(&ref_row.ext_id, Some(board_id))?;
            let already_in_dest = cur_list_id == dest_list.id || normalize_match(&cur_list_name) == norm_dest;

            if already_in_dest {
                set_fixbug_ref_state(conn, ref_row.id, "rejected", Some(clean_reason))
                    .map_err(|e| FixbugError::Other(e.to_string()))?;
                results.push(WritebackItemResult {
                    key: clean_key.to_string(),
                    state: "rejected".to_string(),
                    message: format!("Thẻ đã nằm trong list '{}', chỉ cập nhật trạng thái", dest_list.name),
                });
            } else {
                let comment_text = format!("Từ chối sửa: {clean_reason}");
                client.add_comment(&ref_row.ext_id, &comment_text, &t_cfg.comment_prefix)?;
                client.move_card(&ref_row.ext_id, &dest_list.id)?;
                set_fixbug_ref_state(conn, ref_row.id, "rejected", Some(clean_reason))
                    .map_err(|e| FixbugError::Other(e.to_string()))?;
                results.push(WritebackItemResult {
                    key: clean_key.to_string(),
                    state: "rejected".to_string(),
                    message: format!("Đã chuyển thẻ sang list '{}' và thêm comment lý do", dest_list.name),
                });
            }
        } else if clean_key.starts_with("S-") {
            set_fixbug_ref_state(conn, ref_row.id, "rejected", Some(clean_reason))
                .map_err(|e| FixbugError::Other(e.to_string()))?;
            results.push(WritebackItemResult {
                key: clean_key.to_string(),
                state: "rejected".to_string(),
                message: "Đã cập nhật trạng thái local thành 'rejected'. Nhắc: hãy cập nhật cột Trạng thái trên Google Sheet thủ công.".to_string(),
            });
        } else {
            return Err(FixbugError::Other(format!(
                "Key '{clean_key}' không hợp lệ (phải bắt đầu bằng T- hoặc S-)"
            )));
        }
    }

    Ok(results)
}
