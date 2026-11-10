//! Trạng thái xuất nhật ký (`journal_state`, khóa → giá trị) và hộp thư Telegram
//! (`journal_mail`, gửi đúng một lần nhờ `dedupe_key` UNIQUE). Chính sách lỗi đẩy (đếm, báo,
//! phục hồi) là phần của 07-05.

use super::{ExportOutcome, ExportReport};
use crate::error::BrainResult;
use rusqlite::{params, Connection, OptionalExtension};

pub const KEY_LAST_EXPORT_MS: &str = "last_export_ms";
pub const KEY_LAST_EXPORT_DAY: &str = "last_export_day";
pub const KEY_LAST_PUBLISHED_MS: &str = "last_published_ms";
pub const KEY_LAST_PUBLISHED_DAY: &str = "last_published_day";
pub const KEY_LAST_PUSHED_SHA: &str = "last_pushed_sha";

pub fn get(conn: &Connection, key: &str) -> BrainResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM journal_state WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str, now_ms: i64) -> BrainResult<()> {
    conn.execute(
        "INSERT INTO journal_state (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_ms],
    )?;
    Ok(())
}

/// Xếp một tin vào hộp thư; `false` khi `dedupe_key` đã có (không gửi trùng, kể cả sau restart).
pub fn enqueue_mail(
    conn: &Connection,
    kind: &str,
    dedupe_key: &str,
    text: &str,
    now_ms: i64,
) -> BrainResult<bool> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO journal_mail (kind, dedupe_key, text, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![kind, dedupe_key, text, now_ms],
    )?;
    Ok(n == 1)
}

/// Ghi kết quả một lần xuất: mọi kết quả trừ `Busy` cập nhật `last_export_*`; `Pushed` và
/// `UpToDate` cập nhật `last_published_*`; `Pushed` ghi `last_pushed_sha`.
pub fn record_outcome(
    conn: &Connection,
    cfg: &super::config::JournalConfig,
    report: &ExportReport,
    now_ms: i64,
) -> BrainResult<()> {
    let _ = cfg;
    if report.outcome == ExportOutcome::Busy {
        return Ok(());
    }
    let day = report.today.format("%Y-%m-%d").to_string();
    let now = now_ms.to_string();
    set(conn, KEY_LAST_EXPORT_MS, &now, now_ms)?;
    set(conn, KEY_LAST_EXPORT_DAY, &day, now_ms)?;
    match &report.outcome {
        ExportOutcome::Pushed { sha } => {
            set(conn, KEY_LAST_PUBLISHED_MS, &now, now_ms)?;
            set(conn, KEY_LAST_PUBLISHED_DAY, &day, now_ms)?;
            set(conn, KEY_LAST_PUSHED_SHA, sha, now_ms)?;
        }
        ExportOutcome::UpToDate => {
            set(conn, KEY_LAST_PUBLISHED_MS, &now, now_ms)?;
            set(conn, KEY_LAST_PUBLISHED_DAY, &day, now_ms)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::config::JournalConfig;
    use crate::journal::gitsync::{FailCategory, GitFailure};
    use chrono::NaiveDate;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        (dir, conn)
    }

    fn report(outcome: ExportOutcome, day: u32) -> ExportReport {
        ExportReport {
            outcome,
            today: NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
            today_summary: None,
            changed: Vec::new(),
            removed: Vec::new(),
            foreign: Vec::new(),
        }
    }

    #[test]
    fn record_outcome_sets_export_keys() {
        let (_d, conn) = db();
        let cfg = JournalConfig::default();
        let pushed = report(
            ExportOutcome::Pushed {
                sha: "abc123".to_string(),
            },
            15,
        );
        record_outcome(&conn, &cfg, &pushed, 1000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-15")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_DAY).unwrap().as_deref(),
            Some("2026-01-15")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUSHED_SHA).unwrap().as_deref(),
            Some("abc123")
        );

        let failed = report(
            ExportOutcome::PushFailed(GitFailure {
                category: FailCategory::Network,
                summary: FailCategory::Network.label().to_string(),
            }),
            16,
        );
        record_outcome(&conn, &cfg, &failed, 2000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("2000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-16")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUBLISHED_MS).unwrap().as_deref(),
            Some("1000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_PUSHED_SHA).unwrap().as_deref(),
            Some("abc123")
        );

        let busy = report(ExportOutcome::Busy, 17);
        record_outcome(&conn, &cfg, &busy, 3000).unwrap();
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_MS).unwrap().as_deref(),
            Some("2000")
        );
        assert_eq!(
            get(&conn, KEY_LAST_EXPORT_DAY).unwrap().as_deref(),
            Some("2026-01-16")
        );
    }

    #[test]
    fn enqueue_mail_dedupes() {
        let (_d, conn) = db();
        assert!(enqueue_mail(&conn, "digest", "digest:2026-01-15", "a", 1).unwrap());
        assert!(!enqueue_mail(&conn, "digest", "digest:2026-01-15", "b", 2).unwrap());
        let n: i64 = conn
            .query_row("SELECT count(*) FROM journal_mail", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
