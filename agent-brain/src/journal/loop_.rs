//! Vòng lặp nền của `agent-brain serve` (D-09): xuất nhật ký định kỳ và xếp bản tin cuối ngày.
//! Quy tắc "khi nào xuất" là hàm thuần (`export_due`) để thử bằng đồng hồ giả.

use super::collect::local_day;
use super::config::{JournalConfig, JournalTz};
use super::digest::{digest_due, digest_key, digest_link, digest_moment_ms, digest_text, queued};
use super::{state, ExportOutcome, ExportParams};
use crate::dashboard::AppState;
use crate::error::BrainResult;
use chrono::{NaiveDate, TimeZone};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Nhịp kiểm tra của vòng lặp (độ chi tiết lập lịch, như `detect::SAFETY_SCAN_SECS`); chu kỳ
/// người dùng thấy là `journal.interval_minutes` và `journal.digest_time`.
pub const LOOP_TICK_SECS: u64 = 60;

/// Khóa `journal_state`: số nhịp đã xong (chẩn đoán, cũng để kiểm thử biết một nhịp đã chạy).
pub const KEY_LOOP_TICKS: &str = "loop_ticks";

/// Có nên xuất lúc này không: chưa từng xuất; đã quá `interval_minutes` từ lần xuất trước;
/// sang ngày mới so với lần xuất trước (chốt ngày hôm qua); hoặc đã đến giờ bản tin mà lần xuất
/// trước còn trước giờ đó (bản tin chỉ gửi ngay sau một lần xuất của chính ngày đó).
pub fn export_due<Tz: TimeZone>(
    now_ms: i64,
    tz: &Tz,
    cfg: &JournalConfig,
    last_export_ms: Option<i64>,
    last_export_day: Option<NaiveDate>,
    digest_pending: bool,
) -> bool {
    let Some(last) = last_export_ms else {
        return true;
    };
    if now_ms - last >= cfg.interval_minutes as i64 * 60_000 {
        return true;
    }
    let today = local_day(now_ms, tz);
    if last_export_day != Some(today) {
        return true;
    }
    digest_pending && last < digest_moment_ms(today, tz, cfg)
}

/// Trí nhớ của vòng lặp trong một tiến trình serve.
#[derive(Debug, Default)]
pub struct LoopMem {
    /// Các ngày đã in cảnh báo "bỏ digest" (mỗi ngày một lần).
    dropped_warned: HashSet<NaiveDate>,
}

fn parse_i64(v: Option<String>) -> Option<i64> {
    v.and_then(|s| s.trim().parse().ok())
}

fn parse_day(v: Option<String>) -> Option<NaiveDate> {
    v.and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
}

fn tick_with_tz<Tz: TimeZone>(
    st: &AppState,
    conn: &rusqlite::Connection,
    tz: &Tz,
    mem: &mut LoopMem,
) -> BrainResult<()> {
    let cfg = &st.cfg.journal;
    let map = crate::projects::load_map(&st.map_path)?;
    let now = super::now_ms();
    let today = local_day(now, tz);
    let last_ms = parse_i64(state::get(conn, state::KEY_LAST_EXPORT_MS)?);
    let last_day = parse_day(state::get(conn, state::KEY_LAST_EXPORT_DAY)?);

    // Sang ngày mới mà bản tin hôm qua chưa được xếp hàng: không gửi muộn (D-20), chỉ báo.
    if cfg.digest_enabled {
        if let (Some(yesterday), Some(last)) = (today.pred_opt(), last_day) {
            if last == yesterday
                && !queued(conn, &digest_key(yesterday))?
                && mem.dropped_warned.insert(yesterday)
            {
                crate::term::emit(&format!(
                    "Nhật ký: bỏ digest ngày {} — chưa đẩy được trước 24:00",
                    yesterday.format("%d/%m/%Y")
                ));
            }
        }
    }

    let queued_today = queued(conn, &digest_key(today))?;
    let digest_pending = digest_due(now, tz, cfg, queued_today);
    if !export_due(now, tz, cfg, last_ms, last_day, digest_pending) {
        return Ok(());
    }

    let secrets = crate::inputs::collect_secrets(st);
    let lock_dir = crate::config::data_dir();
    let params = ExportParams {
        conn,
        map: &map,
        cfg,
        secrets: &secrets,
        now_ms: now,
        push: true,
        lock_dir: &lock_dir,
        kill_grace: Duration::from_secs(st.cfg.kill_grace_secs),
    };
    let report = super::run_export_once(&params)?;
    match &report.outcome {
        ExportOutcome::Pushed { .. } | ExportOutcome::UpToDate => {
            // Bản tin chỉ xếp hàng khi liên kết đã chắc chắn có trên remote (D-20).
            if digest_pending {
                if let Some(summary) = &report.today_summary {
                    let link = digest_link(cfg, summary);
                    let text = digest_text(summary, cfg, link.as_deref());
                    state::enqueue_mail(conn, "digest", &digest_key(report.today), &text, now)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Một nhịp của vòng lặp: xuất khi đến hạn, xếp bản tin sau khi đẩy thành công. Bộ đếm
/// `loop_ticks` tăng khi nhịp kết thúc (kể cả khi lỗi), nên "đã tăng" nghĩa là một nhịp trọn vẹn.
pub fn tick(st: &AppState, mem: &mut LoopMem) -> BrainResult<()> {
    let conn = crate::db::open(&st.db_path)?;
    let res = match st.cfg.journal.tz() {
        JournalTz::Local => tick_with_tz(st, &conn, &chrono::Local, mem),
        JournalTz::Fixed(off) => tick_with_tz(st, &conn, &off, mem),
    };
    let ticks = parse_i64(state::get(&conn, KEY_LOOP_TICKS)?).unwrap_or(0) + 1;
    state::set(&conn, KEY_LOOP_TICKS, &ticks.to_string(), super::now_ms())?;
    res
}

/// Vòng lặp nền, khởi động từ `dashboard::cmd_serve` cạnh vòng lặp orchestrator. Tắt (kết thúc
/// ngay) khi `journal.enabled` không bật hoặc thiếu `repo_url`. Mỗi nhịp chạy trong
/// `spawn_blocking` (git có thể chậm); lỗi chỉ ghi log, không làm chết server.
pub fn spawn_loop(st: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !st.cfg.journal.enabled || st.cfg.journal.repo_url.is_none() {
            return;
        }
        let mut shutdown = st.shutdown.clone();
        let mem = Arc::new(Mutex::new(LoopMem::default()));
        loop {
            if *shutdown.borrow() {
                return;
            }
            let (st2, mem2) = (st.clone(), mem.clone());
            let res = tokio::task::spawn_blocking(move || {
                let mut m = mem2.lock().unwrap_or_else(|p| p.into_inner());
                tick(&st2, &mut m)
            })
            .await;
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => crate::term::emit(&format!("Nhật ký: lỗi vòng lặp: {}", e.message)),
                Err(join_err) => crate::term::emit(&format!("Nhật ký: lỗi luồng: {join_err}")),
            }
            let sleep = tokio::time::sleep(Duration::from_secs(LOOP_TICK_SECS));
            tokio::pin!(sleep);
            loop {
                tokio::select! {
                    _ = &mut sleep => break,
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn tz() -> FixedOffset {
        FixedOffset::east_opt(7 * 3600).unwrap()
    }

    fn at(d: u32, h: u32, m: u32) -> i64 {
        tz().with_ymd_and_hms(2026, 1, d, h, m, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    fn day(d: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(2026, 1, d)
    }

    #[test]
    fn export_due_cases() {
        let cfg = JournalConfig::default(); // interval 30, digest 18:00
        let t = tz();
        // never exported
        assert!(export_due(at(15, 10, 0), &t, &cfg, None, None, false));
        // 10 minutes ago, same day
        assert!(!export_due(
            at(15, 10, 10),
            &t,
            &cfg,
            Some(at(15, 10, 0)),
            day(15),
            false
        ));
        // 30 minutes ago
        assert!(export_due(
            at(15, 10, 30),
            &t,
            &cfg,
            Some(at(15, 10, 0)),
            day(15),
            false
        ));
        // day change, only 15 minutes since the last export
        assert!(export_due(
            at(16, 0, 5),
            &t,
            &cfg,
            Some(at(15, 23, 50)),
            day(15),
            false
        ));
        // digest pending, last export before the digest moment
        assert!(export_due(
            at(15, 18, 1),
            &t,
            &cfg,
            Some(at(15, 17, 50)),
            day(15),
            true
        ));
        // digest pending, last export already after the digest moment
        assert!(!export_due(
            at(15, 18, 10),
            &t,
            &cfg,
            Some(at(15, 18, 1)),
            day(15),
            true
        ));
        // digest not pending (already queued or disabled): no extra export at 18:00
        assert!(!export_due(
            at(15, 18, 1),
            &t,
            &cfg,
            Some(at(15, 17, 50)),
            day(15),
            false
        ));
    }
}
