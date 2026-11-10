//! Lệnh `/tim <từ khóa>` (Phase 7, D-15, JRN-06): tìm trong chỉ mục nhật ký đã allowlist
//! (`journal_fts`), chỉ trả ngày + dòng hiển thị + link github.com tới tệp ngày. Chỉ chat đã ghép
//! (kiểm tra ghép nằm ở `bot::authz`, trước khi lệnh tới đây). Không đụng plan, báo cáo QA, diff hay
//! nội dung yêu cầu ngoài những gì vault đã hiển thị.

use super::bot;
use super::render::{clip, MAX_TEXT};
use super::BotCtx;
use crate::journal::config::{blob_url, JournalConfig};
use crate::journal::fts::{self, Hit};
use crate::journal::model::day_path;
use chrono::NaiveDate;
use std::sync::Arc;

pub const USAGE_TIM: &str = "Cách dùng: /tim <từ khóa>";
pub const NOT_FOUND_PREFIX: &str = "Không tìm thấy mục nào trong nhật ký cho: ";
pub const EMPTY_INDEX: &str = "Chưa có nhật ký để tìm — chạy agent-brain journal export.";
const DB_ERROR: &str = "Lỗi CSDL — xem terminal đang chạy agent-brain serve";

/// Câu trả lời: dòng đầu đếm kết quả, rồi mỗi kết quả hai dòng — `DD/MM/YYYY · {display}` và link
/// (hoặc đường dẫn tương đối trong vault khi không suy ra được địa chỉ github.com).
pub fn format_reply(cfg: &JournalConfig, query_folded: &str, hits: &[Hit]) -> String {
    let mut lines = vec![format!(
        "Nhật ký — {} kết quả cho: {query_folded}",
        hits.len()
    )];
    for h in hits {
        match NaiveDate::parse_from_str(&h.day, "%Y-%m-%d") {
            Ok(day) => {
                lines.push(format!("{} · {}", day.format("%d/%m/%Y"), h.display));
                let rel = day_path(day);
                lines.push(blob_url(cfg, &rel).unwrap_or(rel));
            }
            Err(_) => lines.push(format!("{} · {}", h.day, h.display)),
        }
    }
    lines.join("\n")
}

/// `/tim <từ khóa>`: chạy ngay trên luồng poll (chỉ đọc, nhanh), như `/bugs`.
pub fn on_tim(ctx: &Arc<BotCtx>, rest: &str) {
    let tokens = fts::query_tokens(rest);
    if tokens.is_empty() {
        return bot::reply(ctx, USAGE_TIM);
    }
    let conn = match crate::db::open(&ctx.st.db_path) {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&format!("Telegram: /tim — lỗi CSDL: {}", e.message));
            return bot::reply(ctx, DB_ERROR);
        }
    };
    let jcfg = &ctx.st.cfg.journal;
    let found = fts::index_is_empty(&conn).and_then(|empty| {
        if empty {
            Ok(None)
        } else {
            fts::search(&conn, rest, jcfg.search_max_results).map(Some)
        }
    });
    let folded = tokens.join(" ");
    let text = match found {
        Err(e) => {
            crate::term::emit(&format!("Telegram: /tim — lỗi CSDL: {}", e.message));
            DB_ERROR.to_string()
        }
        Ok(None) => EMPTY_INDEX.to_string(),
        Ok(Some(hits)) if hits.is_empty() => format!("{NOT_FOUND_PREFIX}{folded}"),
        Ok(Some(hits)) => format_reply(jcfg, &folded, &hits),
    };
    bot::reply(ctx, &clip(&text, MAX_TEXT));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits() -> Vec<Hit> {
        vec![Hit {
            day: "2026-01-15".to_string(),
            display: "du-an-a — Sửa lỗi đăng nhập trang quản trị · xong".to_string(),
        }]
    }

    #[test]
    fn format_reply_lines() {
        let cfg = JournalConfig {
            web_base_url: Some("https://github.com/example-owner/example-vault".to_string()),
            ..JournalConfig::default()
        };
        assert_eq!(cfg.branch, "main");
        let text = format_reply(&cfg, "dang nhap", &hits());
        assert_eq!(
            text,
            "Nhật ký — 1 kết quả cho: dang nhap\n\
             15/01/2026 · du-an-a — Sửa lỗi đăng nhập trang quản trị · xong\n\
             https://github.com/example-owner/example-vault/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/01/2026-01-15.md"
        );
    }

    #[test]
    fn format_reply_without_web_base() {
        let cfg = JournalConfig {
            repo_url: Some("/tmp/v.git".to_string()),
            web_base_url: None,
            ..JournalConfig::default()
        };
        let text = format_reply(&cfg, "dang nhap", &hits());
        assert_eq!(text.lines().last(), Some("Nhật ký/2026/01/2026-01-15.md"));
    }
}
