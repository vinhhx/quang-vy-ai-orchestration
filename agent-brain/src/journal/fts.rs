//! Chỉ mục FTS5 `journal_fts` cho `/tim` (D-15): chỉ nội dung đã allowlist của nhật ký (tiêu đề,
//! dự án, nhóm, ngày, trạng thái), gấp dấu tiếng Việt trong Rust (hàm `normalize` của routing, đổi cả
//! `đ`) vì tokenizer `unicode61` không gấp `đ`. Dựng lại toàn bộ sau mỗi lần xuất.

use super::model::{weekday_vi, Model};
use crate::error::BrainResult;
use rusqlite::{params, Connection, TransactionBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtsRow {
    pub task_ref: String,
    pub day: String,
    /// Dòng hiển thị (không gấp dấu), đúng như vault: `{slug} — {tiêu đề} · {trạng thái}`.
    pub display: String,
    pub title: String,
    pub project: String,
    pub grp: String,
    pub day_text: String,
    pub status: String,
}

pub fn fold(text: &str) -> String {
    crate::routing::normalize(text)
}

/// Một dòng cho mỗi mục (task, ngày) của mô hình.
pub fn rows_from_model(model: &Model) -> Vec<FtsRow> {
    let mut out = Vec::new();
    for (day, tasks) in &model.days {
        let iso = day.format("%Y-%m-%d").to_string();
        let day_text = format!("{iso} {} {}", day.format("%d/%m/%Y"), weekday_vi(*day));
        for t in tasks {
            out.push(FtsRow {
                task_ref: t.task_id.to_string(),
                day: iso.clone(),
                display: format!("{} — {} · {}", t.project.slug, t.title, t.status.label),
                title: fold(t.title.as_str()),
                project: fold(&t.project.slug),
                grp: fold(&t.project.group_label),
                day_text: fold(&day_text),
                status: fold(&t.status.label),
            });
        }
    }
    out
}

/// Xóa và chèn lại toàn bộ trong một giao dịch IMMEDIATE.
pub fn rebuild(conn: &Connection, rows: &[FtsRow]) -> BrainResult<()> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM journal_fts", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO journal_fts (task_ref, day, display, title, project, grp, day_text, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for r in rows {
            stmt.execute(params![
                r.task_ref, r.day, r.display, r.title, r.project, r.grp, r.day_text, r.status
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        (dir, conn)
    }

    fn row(id: i64, title: &str) -> FtsRow {
        FtsRow {
            task_ref: id.to_string(),
            day: "2026-01-15".to_string(),
            display: format!("du-an-a — {title} · xong"),
            title: fold(title),
            project: fold("du-an-a"),
            grp: fold("Công việc"),
            day_text: fold("2026-01-15 15/01/2026 Thứ Năm"),
            status: fold("xong"),
        }
    }

    fn rows() -> Vec<FtsRow> {
        vec![
            row(1, "Sửa lỗi đăng nhập trang quản trị"),
            row(2, "Tìm kiếm khách hàng theo số điện thoại"),
        ]
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM journal_fts", [], |r| r.get(0))
            .unwrap()
    }

    fn matches(conn: &Connection, q: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT task_ref FROM journal_fts WHERE journal_fts MATCH ?1 ORDER BY task_ref",
            )
            .unwrap();
        stmt.query_map(params![q], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn rebuild_is_idempotent() {
        let (_d, conn) = db();
        rebuild(&conn, &rows()).unwrap();
        rebuild(&conn, &rows()).unwrap();
        assert_eq!(count(&conn), 2);
    }

    #[test]
    fn folded_match_finds_vietnamese() {
        let (_d, conn) = db();
        rebuild(&conn, &rows()).unwrap();
        assert_eq!(matches(&conn, "\"dang\" \"nhap\""), vec!["1"]);
        assert_eq!(matches(&conn, "\"tim\""), vec!["2"]);
        assert_eq!(matches(&conn, "\"kiem\"*"), vec!["2"]);
    }
}
