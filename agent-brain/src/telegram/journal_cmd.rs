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

/// Một kết quả: `DD/MM/YYYY · {display}` và link (hoặc đường dẫn tương đối trong vault khi không
/// suy ra được địa chỉ github.com).
fn result_text(cfg: &JournalConfig, h: &Hit) -> String {
    match NaiveDate::parse_from_str(&h.day, "%Y-%m-%d") {
        Ok(day) => {
            let rel = day_path(day);
            format!(
                "{} · {}\n{}",
                day.format("%d/%m/%Y"),
                h.display,
                blob_url(cfg, &rel).unwrap_or(rel)
            )
        }
        Err(_) => format!("{} · {}", h.day, h.display),
    }
}

/// Câu trả lời: dòng đầu đếm kết quả, rồi mỗi kết quả hai dòng (`result_text`). Khi tổng độ dài
/// vượt `MAX_TEXT`, dừng ở ranh giới một kết quả (không bao giờ cắt giữa kết quả hay link), dòng đầu
/// chỉ đếm các kết quả được liệt kê và dòng cuối là `… và N kết quả khác` (IN-08).
pub fn format_reply(cfg: &JournalConfig, query_folded: &str, hits: &[Hit]) -> String {
    let results: Vec<String> = hits.iter().map(|h| result_text(cfg, h)).collect();
    let build = |shown: usize| {
        let mut lines = vec![format!("Nhật ký — {shown} kết quả cho: {query_folded}")];
        lines.extend(results[..shown].iter().cloned());
        if shown < results.len() {
            lines.push(format!("… và {} kết quả khác", results.len() - shown));
        }
        lines.join("\n")
    };
    (0..=results.len())
        .rev()
        .map(build)
        .find(|t| t.chars().count() <= MAX_TEXT)
        .unwrap_or_else(|| build(0))
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
    fn format_reply_stops_at_a_result_boundary() {
        let cfg = JournalConfig {
            web_base_url: Some("https://github.com/example-owner/example-vault".to_string()),
            ..JournalConfig::default()
        };
        let many: Vec<Hit> = (0..20)
            .map(|i| Hit {
                day: format!("2026-01-{:02}", i + 1),
                display: format!("du-an-a — {} · xong", format!("việc dài {i} ").repeat(30)),
            })
            .collect();
        let text = format_reply(&cfg, "viec", &many);
        assert!(text.chars().count() <= MAX_TEXT, "{}", text.chars().count());
        let lines: Vec<&str> = text.lines().collect();
        let shown = lines.iter().filter(|l| l.starts_with("https://")).count();
        assert!(shown >= 1 && shown < 20, "{shown}");
        assert_eq!(
            lines[0],
            format!("Nhật ký — {shown} kết quả cho: viec"),
            "{}",
            lines[0]
        );
        assert_eq!(
            lines.last().copied(),
            Some(format!("… và {} kết quả khác", 20 - shown).as_str())
        );
        // every listed result is whole: a line with a date is followed by a complete link
        for (i, l) in lines.iter().enumerate().skip(1) {
            if l.contains(" · du-an-a") || l.contains("· du-an-a") {
                assert!(lines[i + 1].ends_with(".md"), "{}", lines[i + 1]);
            }
        }
        // a short list is untouched, with no trailer
        let few = format_reply(&cfg, "viec", &many[..2]);
        assert!(few.starts_with("Nhật ký — 2 kết quả cho: viec\n"), "{few}");
        assert!(!few.contains("kết quả khác"), "{few}");
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
