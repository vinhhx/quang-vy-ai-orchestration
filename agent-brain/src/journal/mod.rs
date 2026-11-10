//! Nhật ký công việc sang vault Obsidian (Phase 7). Một chiều: brain.db → tệp Markdown → git
//! remote riêng tư (D-01). Không đọc lại vault, không gọi model (D-04).
//!
//! Luồng một lần xuất: collect (một giao dịch đọc) → render → cổng secret → gitsync::prepare →
//! writer::apply → gitsync::commit_and_push.

pub mod collect;
pub mod config;
pub mod fts;
pub mod gitsync;
pub mod model;
pub mod render;
pub mod sanitize;
pub mod state;
pub mod writer;

use crate::error::{BrainError, BrainResult};
use chrono::{NaiveDate, TimeZone};
use config::{JournalConfig, JournalTz};
use gitsync::{GitFailure, GitStop};
use model::DaySummary;
use rusqlite::Connection;
use sanitize::SecretHit;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

/// Móc kiểm thử: thời điểm "bây giờ" (epoch ms), giống các biến `AGENT_BRAIN_TELEGRAM_*`.
pub const NOW_ENV: &str = "AGENT_BRAIN_JOURNAL_NOW_MS";

/// Khóa độc quyền của lần xuất, trong thư mục dữ liệu.
pub const LOCK_FILE: &str = "journal.lock";

/// Giữ khóa `journal.lock` (flock, tự nhả khi đóng tệp hoặc tiến trình chết).
#[derive(Debug)]
pub struct JournalLock {
    _file: std::fs::File,
}

/// Lấy khóa độc quyền trong `dir` (cùng kỹ thuật `dashboard/lock.rs`): thư mục 0700, tệp
/// 0600, `flock(LOCK_EX | LOCK_NB)`. `None` khi đang có lần xuất khác giữ khóa.
pub fn acquire_lock(dir: &Path) -> BrainResult<Option<JournalLock>> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    use std::os::unix::io::AsRawFd;
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join(LOCK_FILE))?;
    let res = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if res != 0 {
        let err = std::io::Error::last_os_error();
        let raw = err.raw_os_error();
        if raw == Some(libc::EWOULDBLOCK) || raw == Some(libc::EAGAIN) {
            return Ok(None);
        }
        return Err(err.into());
    }
    Ok(Some(JournalLock { _file: file }))
}

pub fn now_ms() -> i64 {
    if let Ok(v) = std::env::var(NOW_ENV) {
        if let Ok(n) = v.trim().parse::<i64>() {
            return n;
        }
    }
    crate::db::now_ms()
}

pub struct ExportParams<'a> {
    pub conn: &'a Connection,
    pub map: &'a Value,
    pub cfg: &'a JournalConfig,
    pub secrets: &'a [String],
    pub now_ms: i64,
    pub push: bool,
    pub lock_dir: &'a Path,
    pub kill_grace: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportOutcome {
    Pushed { sha: String },
    UpToDate,
    LocalOnly,
    PushFailed(GitFailure),
    Stopped(GitStop),
    SecretBlocked(Vec<SecretHit>),
    Busy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportReport {
    pub outcome: ExportOutcome,
    pub today: NaiveDate,
    pub today_summary: Option<DaySummary>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub foreign: Vec<String>,
}

impl ExportReport {
    fn bare(outcome: ExportOutcome, today: NaiveDate, summary: Option<DaySummary>) -> Self {
        ExportReport {
            outcome,
            today,
            today_summary: summary,
            changed: Vec::new(),
            removed: Vec::new(),
            foreign: Vec::new(),
        }
    }
}

fn local_stamp<Tz: TimeZone>(ms: i64, tz: &Tz) -> String {
    match tz.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.naive_local().format("%Y-%m-%d %H:%M").to_string(),
        None => chrono::DateTime::from_timestamp_millis(ms)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH)
            .naive_utc()
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    }
}

fn export_with_tz<Tz: TimeZone>(p: &ExportParams, tz: &Tz) -> BrainResult<ExportReport> {
    let cfg = p.cfg;
    let today = collect::local_day(p.now_ms, tz);
    let model = collect::collect(p.conn, p.map, cfg, p.secrets, today, tz)?;
    let summary = Some(model::summarize(&model, today));
    let files = render::render_all(&model, cfg);

    let hits = sanitize::scan_generated(&files, p.secrets);
    if !hits.is_empty() {
        return Ok(ExportReport::bare(
            ExportOutcome::SecretBlocked(hits),
            today,
            summary,
        ));
    }

    let ctx = gitsync::GitCtx::from_cfg(cfg, p.kill_grace);
    let generated: BTreeSet<String> = files.keys().cloned().collect();
    match gitsync::prepare(&ctx, cfg, &generated) {
        gitsync::Prepared::Ready => {}
        gitsync::Prepared::Stopped(s) => {
            return Ok(ExportReport::bare(
                ExportOutcome::Stopped(s),
                today,
                summary,
            ))
        }
        gitsync::Prepared::Failed(f) => {
            return Ok(ExportReport::bare(
                ExportOutcome::PushFailed(f),
                today,
                summary,
            ))
        }
    }

    let written = writer::apply(&cfg.clone_dir, &files)?;
    fts::rebuild(p.conn, &fts::rows_from_model(&model))?;
    let mut paths: Vec<String> = written.changed.clone();
    paths.extend(written.removed.iter().cloned());
    let message = format!(
        "journal: {} ({} tệp)",
        local_stamp(p.now_ms, tz),
        paths.len()
    );
    let outcome = match gitsync::commit_and_push(&ctx, cfg, &paths, &message, p.push) {
        gitsync::Published::Pushed { sha } => ExportOutcome::Pushed { sha },
        gitsync::Published::UpToDate => ExportOutcome::UpToDate,
        gitsync::Published::LocalOnly => ExportOutcome::LocalOnly,
        gitsync::Published::CommittedNotPushed(f) => ExportOutcome::PushFailed(f),
    };
    Ok(ExportReport {
        outcome,
        today,
        today_summary: summary,
        changed: written.changed,
        removed: written.removed,
        foreign: written.foreign,
    })
}

/// Một lần xuất trọn vẹn, độc quyền nhờ `journal.lock` trong `p.lock_dir` (đang bận → `Busy`,
/// không làm gì khác). Ngày tính theo `cfg.tz()` (giờ máy, hoặc độ lệch cấu hình). Mọi kết quả
/// khác `Busy` được ghi vào `journal_state`.
pub fn run_export_once(p: &ExportParams) -> BrainResult<ExportReport> {
    let tz = p.cfg.tz();
    let Some(_lock) = acquire_lock(p.lock_dir)? else {
        let today = match tz {
            JournalTz::Local => collect::local_day(p.now_ms, &chrono::Local),
            JournalTz::Fixed(off) => collect::local_day(p.now_ms, &off),
        };
        return Ok(ExportReport::bare(ExportOutcome::Busy, today, None));
    };
    let report = match tz {
        JournalTz::Local => export_with_tz(p, &chrono::Local),
        JournalTz::Fixed(off) => export_with_tz(p, &off),
    }?;
    state::record_outcome(p.conn, p.cfg, &report, p.now_ms)?;
    Ok(report)
}

/// Giá trị secret đã biết: biến môi trường có tên chứa key/token/secret/password, token
/// Telegram và giá trị các biến khóa của engine.
pub fn known_secrets(cfg: &crate::config::Config) -> Vec<String> {
    let mut out = crate::inputs::env_secrets();
    if let Ok(tg) = crate::telegram::config::load(&crate::telegram::config::path()) {
        if tg.token.len() >= crate::secrets::MIN_SECRET_BYTES {
            out.push(tg.token);
        }
    }
    for key_env in cfg.engines.all_key_envs() {
        if let Ok(v) = std::env::var(&key_env) {
            if v.len() >= crate::secrets::MIN_SECRET_BYTES {
                out.push(v);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `agent-brain journal export [--no-push]`
pub fn cmd_export(ctx: &crate::app::Ctx, no_push: bool) -> BrainResult<i32> {
    let cfg = &ctx.cfg.journal;
    if cfg.repo_url.is_none() {
        return Err(BrainError::error(
            "Chưa cấu hình journal.repo_url trong config.json — xem README mục Nhật ký Obsidian.",
        ));
    }
    let secrets = known_secrets(&ctx.cfg);
    let lock_dir = crate::config::data_dir();
    let params = ExportParams {
        conn: &ctx.conn,
        map: &ctx.map,
        cfg,
        secrets: &secrets,
        now_ms: now_ms(),
        push: !no_push,
        lock_dir: &lock_dir,
        kill_grace: Duration::from_secs(ctx.cfg.kill_grace_secs),
    };
    let report = run_export_once(&params)?;
    for f in &report.foreign {
        crate::term::emit(&format!(
            "Cảnh báo: {f} không có dòng đánh dấu tệp sinh tự động — không ghi đè."
        ));
    }
    let n = report.changed.len() + report.removed.len();
    match &report.outcome {
        ExportOutcome::Pushed { sha } => {
            let short: String = sha.chars().take(7).collect();
            crate::term::emit_out(&format!("Nhật ký: đã đẩy {n} tệp ({short})."));
            Ok(0)
        }
        ExportOutcome::UpToDate => {
            crate::term::emit_out("Nhật ký: không có thay đổi.");
            Ok(0)
        }
        ExportOutcome::LocalOnly => {
            let dir = cfg.clone_dir.display();
            crate::term::emit_out(&format!(
                "Nhật ký: đã ghi {n} tệp vào {dir} — chưa commit, chưa đẩy. Xem: git -C {dir} diff"
            ));
            Ok(0)
        }
        ExportOutcome::Busy => {
            crate::term::emit_out("Nhật ký: đang có lần xuất khác — bỏ qua.");
            Ok(0)
        }
        ExportOutcome::PushFailed(f) => Err(BrainError::error(format!(
            "Nhật ký: chưa đẩy được lên repo vault ({}). Commit cục bộ (nếu có) vẫn giữ, lần xuất sau sẽ thử lại.",
            f.summary
        ))),
        ExportOutcome::Stopped(stop) => Err(BrainError::error(match stop {
            GitStop::Conflict { detail } => {
                format!("Nhật ký: dừng — xung đột với repo vault ({detail}); không ghi đè.")
            }
            GitStop::Dirty { paths } => format!(
                "Nhật ký: dừng — bản clone có thay đổi tay: {}; không ghi đè.",
                paths.join(", ")
            ),
            GitStop::NotARepo => format!(
                "Nhật ký: dừng — {} không phải repo git và không rỗng; không xóa, không ghi.",
                cfg.clone_dir.display()
            ),
        })),
        ExportOutcome::SecretBlocked(hits) => {
            let list: Vec<String> = hits
                .iter()
                .map(|h| format!("{} ({})", h.path, h.rule))
                .collect();
            Err(BrainError::error(format!(
                "Nhật ký: dừng — phát hiện giá trị bí mật trong nội dung xuất; không ghi, không commit, không đẩy: {}",
                list.join(", ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = dir.path().join("data");
        let first = acquire_lock(&lock_dir).unwrap();
        assert!(first.is_some());
        assert!(acquire_lock(&lock_dir).unwrap().is_none());
        drop(first);
        assert!(acquire_lock(&lock_dir).unwrap().is_some());
    }

    #[test]
    fn busy_export_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("brain.db")).unwrap();
        let lock_dir = dir.path().join("data");
        let cfg = JournalConfig {
            repo_url: Some(dir.path().join("remote.git").display().to_string()),
            clone_dir: dir.path().join("clone"),
            utc_offset_minutes: Some(420),
            ..JournalConfig::default()
        };
        let map = serde_json::json!({"projects": []});
        let _held = acquire_lock(&lock_dir).unwrap().expect("first lock");
        let params = ExportParams {
            conn: &conn,
            map: &map,
            cfg: &cfg,
            secrets: &[],
            now_ms: 1_768_456_800_000,
            push: true,
            lock_dir: &lock_dir,
            kill_grace: Duration::from_secs(1),
        };
        let report = run_export_once(&params).unwrap();
        assert_eq!(report.outcome, ExportOutcome::Busy);
        assert!(report.changed.is_empty() && report.removed.is_empty());
        assert!(!cfg.clone_dir.exists());
        assert!(state::get(&conn, state::KEY_LAST_EXPORT_MS)
            .unwrap()
            .is_none());
    }
}
