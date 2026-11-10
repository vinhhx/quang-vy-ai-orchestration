//! Bản tin cuối ngày gửi qua Telegram (D-13, D-14, D-20): khi nào gửi và gửi gì. Các hàm ở
//! đây thuần túy, không đọc đồng hồ; thời điểm "bây giờ" do người gọi đưa vào.

use super::config::{blob_url, JournalConfig};
use super::model::{weekday_vi, DaySummary, TODAY_PATH};
use crate::error::BrainResult;
use chrono::{NaiveDate, TimeZone, Timelike};
use rusqlite::{params, Connection};

/// Số dòng "Cần anh xử lý" tối đa trong một bản tin; phần còn lại gộp thành một dòng.
const MAX_ATTENTION_LINES: usize = 10;

/// Khóa chống gửi trùng của bản tin một ngày (`journal_mail.dedupe_key`).
pub fn digest_key(day: NaiveDate) -> String {
    format!("digest:{}", day.format("%Y-%m-%d"))
}

/// Thời điểm (epoch ms) bản tin của ngày `day` đến hạn: `digest_time` theo giờ địa phương.
pub fn digest_moment_ms<Tz: TimeZone>(day: NaiveDate, tz: &Tz, cfg: &JournalConfig) -> i64 {
    let (h, m) = cfg.digest_time;
    let at = day
        .and_hms_opt(h, m, 0)
        .and_then(|t| tz.from_local_datetime(&t).earliest())
        .map(|t| t.timestamp_millis());
    match at {
        Some(ms) => ms,
        // Giờ không tồn tại (nhảy giờ mùa hè): lấy đầu ngày cộng số phút.
        None => {
            let start = super::collect::day_bounds_ms(day, tz).0;
            start + (h as i64 * 60 + m as i64) * 60_000
        }
    }
}

/// Đã đến giờ gửi bản tin hôm nay chưa: bật, giờ địa phương ≥ `digest_time`, chưa xếp hàng.
pub fn digest_due<Tz: TimeZone>(
    now_ms: i64,
    tz: &Tz,
    cfg: &JournalConfig,
    queued_today: bool,
) -> bool {
    if !cfg.digest_enabled || queued_today {
        return false;
    }
    let Some(local) = tz.timestamp_millis_opt(now_ms).single() else {
        return false;
    };
    let t = local.naive_local().time();
    (t.hour(), t.minute()) >= cfg.digest_time
}

/// Liên kết github.com tới tệp của ngày khi ngày đó có tệp (có việc hoặc có mục cần xử lý),
/// ngược lại tới `Today.md`. `None` khi không dựng được liên kết (không phải github.com và
/// không có `web_base_url`).
pub fn digest_link(cfg: &JournalConfig, summary: &DaySummary) -> Option<String> {
    if summary.total > 0 || !summary.attention.is_empty() {
        blob_url(cfg, &summary.path)
    } else {
        blob_url(cfg, TODAY_PATH)
    }
}

/// Nội dung bản tin: văn bản thuần, các dòng nối bằng `\n`.
pub fn digest_text(summary: &DaySummary, cfg: &JournalConfig, link: Option<&str>) -> String {
    let d = summary.day;
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "Nhật ký {} {}: {} việc ({} xong)",
        weekday_vi(d),
        d.format("%d/%m/%Y"),
        summary.total,
        summary.done
    ));
    if !summary.by_group.is_empty() {
        let groups: Vec<String> = summary
            .by_group
            .iter()
            .map(|(label, n)| format!("{label} {n}"))
            .collect();
        lines.push(groups.join(" · "));
    }
    if summary.attention.is_empty() {
        lines.push("Không có việc cần anh xử lý.".to_string());
    } else {
        lines.push(format!("Cần anh xử lý ({}):", summary.attention.len()));
        for item in summary.attention.iter().take(MAX_ATTENTION_LINES) {
            lines.push(format!(
                "- {} — {} — {}",
                item.kind.label(),
                item.project_slug,
                item.title.cut(cfg.callout_title_chars)
            ));
        }
        if summary.attention.len() > MAX_ATTENTION_LINES {
            lines.push(format!(
                "… và {} mục khác",
                summary.attention.len() - MAX_ATTENTION_LINES
            ));
        }
    }
    if let Some(l) = link {
        lines.push(l.to_string());
    }
    lines.join("\n")
}

/// Đã có dòng hộp thư với `key` chưa (kể cả đã gửi hoặc đã bỏ).
pub fn queued(conn: &Connection, key: &str) -> BrainResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM journal_mail WHERE dedupe_key = ?1",
        params![key],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::model::{day_path, AttentionItem, AttentionKind};
    use crate::journal::sanitize::sanitize_text;
    use chrono::FixedOffset;

    fn tz() -> FixedOffset {
        FixedOffset::east_opt(7 * 3600).unwrap()
    }

    fn at(h: u32, m: u32) -> i64 {
        tz().with_ymd_and_hms(2026, 1, 15, h, m, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
    }

    fn cfg() -> JournalConfig {
        JournalConfig {
            web_base_url: Some("https://github.com/example-owner/example-vault".to_string()),
            ..JournalConfig::default()
        }
    }

    fn item(kind: AttentionKind, slug: &str, title: &str) -> AttentionItem {
        AttentionItem {
            task_id: 1,
            project_slug: slug.to_string(),
            title: sanitize_text(title, 200, &[]),
            kind,
            day: day(),
        }
    }

    fn summary(total: usize, attention: Vec<AttentionItem>) -> DaySummary {
        DaySummary {
            day: day(),
            path: day_path(day()),
            total,
            done: if total == 7 { 4 } else { 0 },
            by_group: vec![
                ("Công việc".to_string(), 4),
                ("Video".to_string(), 2),
                ("Hỗ trợ bộ phận khác".to_string(), 1),
            ],
            attention,
        }
    }

    #[test]
    fn digest_due_cases() {
        let c = cfg();
        assert!(!digest_due(at(17, 59), &tz(), &c, false));
        assert!(digest_due(at(18, 0), &tz(), &c, false));
        assert!(digest_due(at(23, 59), &tz(), &c, false));
        assert!(!digest_due(at(18, 30), &tz(), &c, true));
        let off = JournalConfig {
            digest_enabled: false,
            ..cfg()
        };
        assert!(!digest_due(at(18, 30), &tz(), &off, false));
        let early = JournalConfig {
            digest_time: (7, 30),
            ..cfg()
        };
        assert!(digest_due(at(7, 30), &tz(), &early, false));
        assert!(!digest_due(at(7, 29), &tz(), &early, false));
    }

    #[test]
    fn digest_moment_is_local_digest_time() {
        assert_eq!(digest_moment_ms(day(), &tz(), &cfg()), at(18, 0));
    }

    #[test]
    fn digest_text_format() {
        let s = summary(
            7,
            vec![
                item(
                    AttentionKind::PlanWaiting,
                    "du-an-a",
                    "Lập kế hoạch nội dung tuần",
                ),
                item(
                    AttentionKind::Quota,
                    "du-an-c",
                    "Chuẩn bị số liệu báo cáo tuần",
                ),
            ],
        );
        let link = "https://github.com/example-owner/example-vault/blob/main/Nh%E1%BA%ADt%20k%C3%BD/2026/01/2026-01-15.md";
        let text = digest_text(&s, &cfg(), Some(link));
        let expected = [
            "Nhật ký Thứ Năm 15/01/2026: 7 việc (4 xong)",
            "Công việc 4 · Video 2 · Hỗ trợ bộ phận khác 1",
            "Cần anh xử lý (2):",
            "- chờ duyệt plan — du-an-a — Lập kế hoạch nội dung tuần",
            "- hết quota — du-an-c — Chuẩn bị số liệu báo cáo tuần",
            link,
        ]
        .join("\n");
        assert_eq!(text, expected);
        assert_eq!(digest_link(&cfg(), &s).as_deref(), Some(link));
    }

    #[test]
    fn digest_text_without_attention_and_overflow() {
        let none = digest_text(&summary(3, Vec::new()), &cfg(), None);
        assert!(none.contains("Không có việc cần anh xử lý."), "{none}");
        assert!(!none.contains("Cần anh xử lý ("), "{none}");
        assert_eq!(none.lines().count(), 3, "{none}");

        let many: Vec<AttentionItem> = (0..12)
            .map(|i| item(AttentionKind::Failed, "du-an-a", &format!("việc số {i}")))
            .collect();
        let text = digest_text(&summary(12, many), &cfg(), None);
        assert!(text.contains("Cần anh xử lý (12):"), "{text}");
        assert_eq!(text.lines().filter(|l| l.starts_with("- ")).count(), 10);
        assert!(text.lines().any(|l| l == "… và 2 mục khác"), "{text}");
    }

    #[test]
    fn digest_text_cuts_long_titles() {
        let c = JournalConfig {
            callout_title_chars: 20,
            ..cfg()
        };
        let long = "a".repeat(100);
        let text = digest_text(
            &summary(1, vec![item(AttentionKind::Failed, "du-an-a", &long)]),
            &c,
            None,
        );
        let line = text.lines().find(|l| l.starts_with("- ")).unwrap();
        let title = line.rsplit(" — ").next().unwrap();
        assert_eq!(title.chars().count(), 20, "{line}");
        assert!(title.ends_with('…'), "{line}");
    }

    #[test]
    fn digest_link_falls_back_to_today_md() {
        let s = summary(0, Vec::new());
        let link = digest_link(&cfg(), &s).unwrap();
        assert!(link.ends_with("/blob/main/Today.md"), "{link}");
        let attention_only = summary(0, vec![item(AttentionKind::Failed, "du-an-a", "x")]);
        assert!(digest_link(&cfg(), &attention_only)
            .unwrap()
            .ends_with("2026-01-15.md"));
        assert_eq!(digest_link(&JournalConfig::default(), &s), None);
    }

    #[test]
    fn digest_key_is_per_day() {
        assert_eq!(digest_key(day()), "digest:2026-01-15");
    }

    #[test]
    fn queued_sees_any_row_with_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        assert!(!queued(&conn, "digest:2026-01-15").unwrap());
        crate::journal::state::enqueue_mail(&conn, "digest", "digest:2026-01-15", "x", 1).unwrap();
        assert!(queued(&conn, "digest:2026-01-15").unwrap());
        assert!(!queued(&conn, "digest:2026-01-16").unwrap());
    }
}
