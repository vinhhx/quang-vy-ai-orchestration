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

/// Giới hạn độ dài truy vấn `/tim` (ký tự) và số từ khóa; phần thừa bị bỏ (D-15, T-07-26).
pub const MAX_QUERY_CHARS: usize = 100;
pub const MAX_QUERY_TOKENS: usize = 8;

/// Một kết quả tìm kiếm: ngày (`YYYY-MM-DD`) và dòng hiển thị đúng như vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub day: String,
    pub display: String,
}

/// Các từ khóa đã gấp dấu của truy vấn người dùng, tối đa `MAX_QUERY_TOKENS`.
pub fn query_tokens(user: &str) -> Vec<String> {
    let cut: String = user.chars().take(MAX_QUERY_CHARS).collect();
    fold(&cut)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(MAX_QUERY_TOKENS)
        .map(str::to_string)
        .collect()
}

/// Chuỗi MATCH an toàn từ văn bản Telegram không tin cậy: mỗi từ khóa là một chuỗi trong ngoặc kép
/// (không toán tử, không lọc cột), từ cuối cùng khớp tiền tố (trừ từ một ký tự, để `du-an-a` không
/// khớp luôn `an`); các từ nối bằng AND ngầm.
pub fn match_query(user: &str) -> Option<String> {
    let toks = query_tokens(user);
    let last = toks.len().checked_sub(1)?;
    let parts: Vec<String> = toks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let quoted = format!("\"{}\"", t.replace('"', "\"\""));
            if i == last && t.chars().count() >= 2 {
                format!("{quoted}*")
            } else {
                quoted
            }
        })
        .collect();
    Some(parts.join(" "))
}

/// Tìm trong `journal_fts` (chỉ `day` và `display`); rỗng khi truy vấn không có từ khóa.
pub fn search(conn: &Connection, user: &str, limit: usize) -> BrainResult<Vec<Hit>> {
    let Some(q) = match_query(user) else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT day, display FROM journal_fts WHERE journal_fts MATCH ?1
         ORDER BY bm25(journal_fts), day DESC LIMIT ?2",
    )?;
    let hits = stmt
        .query_map(params![q, limit as i64], |r| {
            Ok(Hit {
                day: r.get(0)?,
                display: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hits)
}

pub fn index_is_empty(conn: &Connection) -> BrainResult<bool> {
    let n: i64 = conn.query_row("SELECT count(*) FROM journal_fts", [], |r| r.get(0))?;
    Ok(n == 0)
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

    fn dated_rows() -> Vec<FtsRow> {
        let mk = |id: i64, day: &str, dmy: &str, wd: &str, slug: &str, title: &str| FtsRow {
            task_ref: id.to_string(),
            day: day.to_string(),
            display: format!("{slug} — {title} · xong"),
            title: fold(title),
            project: fold(slug),
            grp: fold("Công việc"),
            day_text: fold(&format!("{day} {dmy} {wd}")),
            status: fold("xong"),
        };
        vec![
            mk(
                1,
                "2026-01-15",
                "15/01/2026",
                "Thứ Năm",
                "du-an-a",
                "Sửa lỗi đăng nhập trang quản trị",
            ),
            mk(
                2,
                "2026-01-14",
                "14/01/2026",
                "Thứ Tư",
                "du-an-b",
                "Tìm kiếm khách hàng theo số điện thoại",
            ),
        ]
    }

    fn days(hits: &[Hit]) -> Vec<String> {
        hits.iter().map(|h| h.day.clone()).collect()
    }

    #[test]
    fn match_query_quotes_tokens() {
        assert_eq!(
            match_query("dang nhap").as_deref(),
            Some("\"dang\" \"nhap\"*")
        );
        assert_eq!(
            match_query("Đăng  NHẬP!!").as_deref(),
            Some("\"dang\" \"nhap\"*")
        );
        assert_eq!(match_query("du a").as_deref(), Some("\"du\" \"a\""));
        assert_eq!(match_query(""), None);
        assert_eq!(match_query("!!! ???"), None);
        let twenty = (1..=20)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(query_tokens(&twenty).len(), MAX_QUERY_TOKENS);
        let long = format!("{} zzzcuoi", "a".repeat(MAX_QUERY_CHARS));
        assert_eq!(query_tokens(&long), vec!["a".repeat(MAX_QUERY_CHARS)]);
        let five_hundred = "ab ".repeat(200);
        assert!(query_tokens(&five_hundred).len() <= MAX_QUERY_TOKENS);
    }

    #[test]
    fn hostile_queries_never_error() {
        let (_d, conn) = db();
        rebuild(&conn, &dated_rows()).unwrap();
        for q in [
            "AND",
            "foo AND",
            "\"unterminated",
            "a-b",
            "col:foo",
            "title:sua",
            "*",
            "(",
            "tim^",
            "NOT x",
            "fix bug #12",
            "50% off",
            "a'b",
            "title:sua AND (",
            "NEAR(a b)",
        ] {
            let r = search(&conn, q, 5);
            assert!(r.is_ok(), "query {q:?} errored: {r:?}");
        }
        assert!(search(&conn, "title:sua", 5).unwrap().is_empty());
    }

    #[test]
    fn search_folds_and_prefixes() {
        let (_d, conn) = db();
        rebuild(&conn, &dated_rows()).unwrap();
        assert_eq!(
            days(&search(&conn, "dang nhap", 5).unwrap()),
            vec!["2026-01-15"]
        );
        assert_eq!(days(&search(&conn, "tim", 5).unwrap()), vec!["2026-01-14"]);
        assert_eq!(days(&search(&conn, "kie", 5).unwrap()), vec!["2026-01-14"]);
        assert_eq!(
            days(&search(&conn, "2026 01 14", 5).unwrap()),
            vec!["2026-01-14"]
        );
        assert_eq!(
            days(&search(&conn, "du-an-a", 5).unwrap()),
            vec!["2026-01-15"]
        );
        assert_eq!(search(&conn, "2026", 1).unwrap().len(), 1);
        assert_eq!(search(&conn, "2026", 5).unwrap().len(), 2);
        let hit = &search(&conn, "dang nhap", 5).unwrap()[0];
        assert_eq!(
            hit.display,
            "du-an-a — Sửa lỗi đăng nhập trang quản trị · xong"
        );
    }

    #[test]
    fn index_is_empty_reports() {
        let (_d, conn) = db();
        assert!(index_is_empty(&conn).unwrap());
        rebuild(&conn, &rows()).unwrap();
        assert!(!index_is_empty(&conn).unwrap());
    }
}
