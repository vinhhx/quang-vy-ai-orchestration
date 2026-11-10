//! Vòng lặp nền của `agent-brain serve` (D-09): xuất nhật ký định kỳ và xếp bản tin cuối ngày.
//! Quy tắc "khi nào xuất" là hàm thuần (`export_due`) để thử bằng đồng hồ giả.

use super::collect::local_day;
use super::config::JournalConfig;
use super::digest::digest_moment_ms;
use chrono::{NaiveDate, TimeZone};

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
