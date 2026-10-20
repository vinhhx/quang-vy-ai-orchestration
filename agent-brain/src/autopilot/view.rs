//! Lời hiển thị khi autopilot dừng — chuỗi do server dựng.

use crate::autopilot::decide::{Loop, StopInfo, StopReason};

/// Từ đại diện lý do dừng (tiếng Việt, chữ thường).
pub fn reason_word(r: StopReason) -> &'static str {
    match r {
        StopReason::HetVong => "hết vòng",
        StopReason::HetNganSach => "hết ngân sách",
        StopReason::ViPhamGiaoThuc => "vi phạm giao thức",
        StopReason::VuongMac => "vướng mắc",
        StopReason::DungBoiBan => "dừng bởi bạn",
        StopReason::RunLoi => "run lỗi",
    }
}

/// Tiêu đề của vòng lặp hiển thị trên giao diện.
pub fn loop_title(l: Loop) -> &'static str {
    match l {
        Loop::Plan => "Review plan",
        Loop::Build => "Review code",
        Loop::Qa => "QA",
    }
}

/// Định dạng số tiền USD với 2 chữ số thập phân.
pub fn fmt_usd(x: f64) -> String {
    format!("${:.2}", x)
}

/// Định dạng timestamp mili-giây sang giờ địa phương HH:MM.
pub fn fmt_hhmm(at_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(at_ms)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string())
}

/// Dựng chuỗi chi tiết khi autopilot dừng từ StopInfo và slug của task.
pub fn stop_detail(info: &StopInfo, slug: &str) -> String {
    match info.reason {
        StopReason::HetVong => match info.loop_name {
            Some(Loop::Plan) => {
                format!(
                    "Review plan vẫn yêu cầu sửa lại sau {} / {} vòng.",
                    info.round.unwrap_or(1),
                    info.cap.unwrap_or(1)
                )
            }
            Some(Loop::Build) => {
                format!(
                    "Review code vẫn yêu cầu sửa lại sau {} / {} vòng.",
                    info.round.unwrap_or(1),
                    info.cap.unwrap_or(1)
                )
            }
            Some(Loop::Qa) | None => {
                format!(
                    "QA vẫn kết luận Cần sửa sau {} / {} vòng. Xem báo cáo QA bên dưới.",
                    info.round.unwrap_or(1),
                    info.cap.unwrap_or(1)
                )
            }
        },
        StopReason::HetNganSach => {
            let used_ms = info.used_ms.unwrap_or(0);
            let minutes = (used_ms as f64 / 60_000.0).round() as i64;
            let limit_minutes = info.limit_minutes.unwrap_or(60);
            let used_usd = info.used_usd.unwrap_or(0.0);
            let limit_usd = info.limit_usd.unwrap_or(20.0);
            let estimated = info.estimated.unwrap_or(false);

            let money_part = if estimated {
                format!("≈ {} / {}", fmt_usd(used_usd), fmt_usd(limit_usd))
            } else {
                format!("{} / {}", fmt_usd(used_usd), fmt_usd(limit_usd))
            };

            format!(
                "Đã dùng {} / {} phút · {}. Run đang chạy đã xong; autopilot không bắt đầu bước nào nữa.",
                minutes, limit_minutes, money_part
            )
        }
        StopReason::ViPhamGiaoThuc => {
            let run_id = info.run_id.unwrap_or(0);
            let role = info.role.as_deref().unwrap_or("run");
            format!(
                "Run {} ({}) vi phạm giao thức — xem log của run này.",
                run_id, role
            )
        }
        StopReason::VuongMac => {
            format!(
                "Dev dừng vì vướng mắc. Sửa .plan.md trong editor của bạn rồi commit với message '{}: plan', hoặc chạy lại Lead.",
                slug
            )
        }
        StopReason::DungBoiBan => {
            format!("Bạn đã dừng autopilot lúc {}.", fmt_hhmm(info.at_ms))
        }
        StopReason::RunLoi => {
            if let Some(id) = info.run_id {
                let role = info.role.as_deref().unwrap_or("run");
                format!(
                    "Run {} ({}) lỗi hoặc quá thời gian — xem log của run này.",
                    id, role
                )
            } else if let Some(ref detail) = info.detail_line {
                detail.clone()
            } else {
                "Autopilot không bắt đầu được bước tiếp theo.".to_string()
            }
        }
    }
}

/// Dòng thông báo dừng hiển thị ở terminal (D-20).
pub fn terminal_line(info: &StopInfo, slug: &str, task_id: i64) -> String {
    format!(
        "Autopilot dừng · {}: {} (task {})",
        reason_word(info.reason),
        stop_detail(info, slug),
        task_id
    )
}

/// Dòng thông báo khi dừng ở cổng duyệt plan.
pub fn gate_line(task_id: i64) -> String {
    format!(
        "Autopilot dừng ở bước duyệt plan (task {}) — duyệt: agent-brain approve {}",
        task_id, task_id
    )
}

/// Dòng thông báo khi autopilot hoàn thành công việc.
pub fn done_line(task_id: i64, qa: bool) -> String {
    if qa {
        format!("Autopilot xong: QA đạt (task {})", task_id)
    } else {
        format!("Autopilot xong (task {})", task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autopilot::decide::{Loop, Stage, StopInfo, StopReason};

    #[test]
    fn reason_words() {
        assert_eq!(reason_word(StopReason::HetVong), "hết vòng");
        assert_eq!(reason_word(StopReason::HetNganSach), "hết ngân sách");
        assert_eq!(reason_word(StopReason::ViPhamGiaoThuc), "vi phạm giao thức");
        assert_eq!(reason_word(StopReason::VuongMac), "vướng mắc");
        assert_eq!(reason_word(StopReason::DungBoiBan), "dừng bởi bạn");
        assert_eq!(reason_word(StopReason::RunLoi), "run lỗi");
    }

    #[test]
    fn het_vong_details() {
        let p_stop = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Plan),
            stage: Some(Stage::PlanReview),
            round: Some(3),
            cap: Some(3),
            run_id: Some(1),
            role: Some("plan-review".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&p_stop, "demo"),
            "Review plan vẫn yêu cầu sửa lại sau 3 / 3 vòng."
        );

        let b_stop = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::CodeReview),
            round: Some(4),
            cap: Some(3),
            run_id: Some(2),
            role: Some("code-review".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&b_stop, "demo"),
            "Review code vẫn yêu cầu sửa lại sau 4 / 3 vòng."
        );

        let q_stop = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Qa),
            stage: Some(Stage::Qa),
            round: Some(1),
            cap: Some(1),
            run_id: Some(3),
            role: Some("qa".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&q_stop, "demo"),
            "QA vẫn kết luận Cần sửa sau 1 / 1 vòng. Xem báo cáo QA bên dưới."
        );
    }

    #[test]
    fn budget_detail() {
        let mut stop = StopInfo {
            reason: StopReason::HetNganSach,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::Implement),
            round: Some(1),
            cap: None,
            run_id: None,
            role: None,
            at_ms: 1000,
            detail_line: None,
            used_ms: Some(3_630_000),
            used_usd: Some(20.4),
            estimated: Some(false),
            limit_minutes: Some(60),
            limit_usd: Some(20.0),
        };

        let suffix = concat!(
            "Run đang chạy đã xong; ",
            "autopilot không bắt đầu bước nào nữa."
        );
        assert_eq!(
            stop_detail(&stop, "demo"),
            format!("Đã dùng 61 / 60 phút · $20.40 / $20.00. {suffix}")
        );

        stop.estimated = Some(true);
        assert_eq!(
            stop_detail(&stop, "demo"),
            format!("Đã dùng 61 / 60 phút · ≈ $20.40 / $20.00. {suffix}")
        );
    }

    #[test]
    fn other_details() {
        // vi phạm run 12 role "plan-review"
        let stop_proto = StopInfo {
            reason: StopReason::ViPhamGiaoThuc,
            loop_name: Some(Loop::Plan),
            stage: Some(Stage::PlanReview),
            round: Some(1),
            cap: None,
            run_id: Some(12),
            role: Some("plan-review".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&stop_proto, "demo"),
            "Run 12 (plan-review) vi phạm giao thức — xem log của run này."
        );

        // vướng mắc slug "sua-app"
        let stop_blocked = StopInfo {
            reason: StopReason::VuongMac,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::Implement),
            round: Some(1),
            cap: None,
            run_id: Some(5),
            role: Some("dev".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&stop_blocked, "sua-app"),
            "Dev dừng vì vướng mắc. Sửa .plan.md trong editor của bạn rồi commit với message 'sua-app: plan', hoặc chạy lại Lead."
        );

        // dừng bởi bạn at_ms t
        let t = 1_700_000_000_000i64;
        let stop_user = StopInfo {
            reason: StopReason::DungBoiBan,
            loop_name: None,
            stage: None,
            round: None,
            cap: None,
            run_id: None,
            role: None,
            at_ms: t,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        let expected_hhmm = fmt_hhmm(t);
        assert_eq!(
            stop_detail(&stop_user, "demo"),
            format!("Bạn đã dừng autopilot lúc {}.", expected_hhmm)
        );

        // run lỗi run 7 role "dev"
        let stop_err_run = StopInfo {
            reason: StopReason::RunLoi,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::Implement),
            round: Some(1),
            cap: None,
            run_id: Some(7),
            role: Some("dev".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&stop_err_run, "demo"),
            "Run 7 (dev) lỗi hoặc quá thời gian — xem log của run này."
        );

        // run lỗi without run_id and detail_line "Working tree có thay đổi chưa commit:"
        let stop_err_detail = StopInfo {
            reason: StopReason::RunLoi,
            loop_name: None,
            stage: None,
            round: None,
            cap: None,
            run_id: None,
            role: None,
            at_ms: 1000,
            detail_line: Some("Working tree có thay đổi chưa commit:".to_string()),
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&stop_err_detail, "demo"),
            "Working tree có thay đổi chưa commit:"
        );

        // without both → "Autopilot không bắt đầu được bước tiếp theo."
        let stop_err_none = StopInfo {
            reason: StopReason::RunLoi,
            loop_name: None,
            stage: None,
            round: None,
            cap: None,
            run_id: None,
            role: None,
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            stop_detail(&stop_err_none, "demo"),
            "Autopilot không bắt đầu được bước tiếp theo."
        );
    }

    #[test]
    fn lines() {
        let info = StopInfo {
            reason: StopReason::HetVong,
            loop_name: Some(Loop::Build),
            stage: Some(Stage::CodeReview),
            round: Some(3),
            cap: Some(3),
            run_id: Some(10),
            role: Some("code-review".to_string()),
            at_ms: 1000,
            detail_line: None,
            used_ms: None,
            used_usd: None,
            estimated: None,
            limit_minutes: None,
            limit_usd: None,
        };
        assert_eq!(
            terminal_line(&info, "demo", 5),
            "Autopilot dừng · hết vòng: Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng. (task 5)"
        );

        assert_eq!(
            gate_line(5),
            "Autopilot dừng ở bước duyệt plan (task 5) — duyệt: agent-brain approve 5"
        );

        assert_eq!(done_line(5, true), "Autopilot xong: QA đạt (task 5)");
        assert_eq!(done_line(5, false), "Autopilot xong (task 5)");
    }

    #[test]
    fn fmt_usd_cases() {
        assert_eq!(fmt_usd(1.32), "$1.32");
        assert_eq!(fmt_usd(20.0), "$20.00");
        assert_eq!(fmt_usd(0.0594874), "$0.06");
    }
}
