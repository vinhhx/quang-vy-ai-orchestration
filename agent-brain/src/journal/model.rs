//! Bố cục vault (D-02) và mô hình dữ liệu trung gian giữa `collect` và `render`.

use super::config::GroupDef;
use super::sanitize::SafeText;
use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::BTreeMap;

/// Dòng đánh dấu tệp sinh tự động (D-17, D-22): chỉ tệp có dòng này mới được ghi đè/xóa.
pub const MARKER: &str =
    "<!-- Tệp sinh tự động từ agent-brain — không sửa tay, lần xuất sau sẽ ghi đè. -->";
pub const JOURNAL_DIR: &str = "Nhật ký";
pub const PROJECTS_DIR: &str = "Dự án";
pub const TODAY_PATH: &str = "Today.md";

/// `Nhật ký/YYYY/MM/YYYY-MM-DD.md`
pub fn day_path(d: NaiveDate) -> String {
    format!(
        "{JOURNAL_DIR}/{:04}/{:02}/{}.md",
        d.year(),
        d.month(),
        d.format("%Y-%m-%d")
    )
}

/// `Nhật ký/YYYY/MM/YYYY-MM.md`
pub fn month_path(year: i32, month: u32) -> String {
    format!("{JOURNAL_DIR}/{year:04}/{month:02}/{year:04}-{month:02}.md")
}

/// `Nhật ký/YYYY/YYYY.md`
pub fn year_path(year: i32) -> String {
    format!("{JOURNAL_DIR}/{year:04}/{year:04}.md")
}

/// `Dự án/{slug}.md` (slug đã qua `collect::project_file_key`).
pub fn project_path(slug: &str) -> String {
    format!("{PROJECTS_DIR}/{slug}.md")
}

pub fn weekday_vi(d: NaiveDate) -> &'static str {
    match d.weekday() {
        Weekday::Mon => "Thứ Hai",
        Weekday::Tue => "Thứ Ba",
        Weekday::Wed => "Thứ Tư",
        Weekday::Thu => "Thứ Năm",
        Weekday::Fri => "Thứ Sáu",
        Weekday::Sat => "Thứ Bảy",
        Weekday::Sun => "Chủ Nhật",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRef {
    /// Tên tệp an toàn của dự án (không phải đường dẫn, không phải `name_vi`).
    pub slug: String,
    pub group_id: String,
    pub group_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttentionKind {
    PlanWaiting,
    Quota,
    EnvDown,
    QaFix,
    Blocked,
    Failed,
    Timeout,
    ProtocolViolation,
}

impl AttentionKind {
    pub fn label(&self) -> &'static str {
        match self {
            AttentionKind::PlanWaiting => "chờ duyệt plan",
            AttentionKind::Quota => "hết quota",
            AttentionKind::EnvDown => "môi trường chưa sẵn sàng",
            AttentionKind::QaFix => "QA cần sửa",
            AttentionKind::Blocked => "vướng mắc",
            AttentionKind::Failed => "lỗi",
            AttentionKind::Timeout => "quá thời gian",
            AttentionKind::ProtocolViolation => "vi phạm giao thức",
        }
    }

    /// 1 = plan chờ duyệt, 2 = quota/môi trường, 3 = QA cần sửa/vướng mắc,
    /// 4 = lỗi/timeout/vi phạm (D-16).
    pub fn category(&self) -> u8 {
        match self {
            AttentionKind::PlanWaiting => 1,
            AttentionKind::Quota | AttentionKind::EnvDown => 2,
            AttentionKind::QaFix | AttentionKind::Blocked => 3,
            AttentionKind::Failed | AttentionKind::Timeout | AttentionKind::ProtocolViolation => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Done,
    Attention(AttentionKind),
    Active,
    Running,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayStatus {
    pub kind: StatusKind,
    pub label: String,
}

/// Một mục (một task trong một ngày, D-06).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDay {
    pub task_id: i64,
    pub day: NaiveDate,
    pub project: ProjectRef,
    pub title: SafeText,
    pub request: SafeText,
    pub status: DayStatus,
    pub runs: u32,
    pub rounds: u32,
    pub roles: Vec<String>,
    pub duration_ms: i64,
    pub first_run_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttentionItem {
    pub task_id: i64,
    pub project_slug: String,
    pub title: SafeText,
    pub kind: AttentionKind,
    pub day: NaiveDate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub today: NaiveDate,
    pub days: BTreeMap<NaiveDate, Vec<TaskDay>>,
    pub attention: BTreeMap<NaiveDate, Vec<AttentionItem>>,
    pub groups: Vec<GroupDef>,
    /// Tên tệp của các dự án có ít nhất một mục, đã sắp xếp.
    pub project_slugs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    pub day: NaiveDate,
    pub path: String,
    pub total: usize,
    pub done: usize,
    /// (nhãn nhóm, số mục) cho mọi nhóm cấu hình, đúng thứ tự cấu hình, kể cả nhóm 0.
    pub by_group: Vec<(String, usize)>,
    pub attention: Vec<AttentionItem>,
}

pub fn summarize(model: &Model, day: NaiveDate) -> DaySummary {
    let tasks: &[TaskDay] = model.days.get(&day).map(Vec::as_slice).unwrap_or(&[]);
    let done = tasks
        .iter()
        .filter(|t| t.status.kind == StatusKind::Done)
        .count();
    let by_group = model
        .groups
        .iter()
        .map(|g| {
            (
                g.label.clone(),
                tasks.iter().filter(|t| t.project.group_id == g.id).count(),
            )
        })
        .collect();
    DaySummary {
        day,
        path: day_path(day),
        total: tasks.len(),
        done,
        by_group,
        attention: model.attention.get(&day).cloned().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    #[test]
    fn layout_paths_are_nfc() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        assert_eq!(day_path(d), "Nhật ký/2026/01/2026-01-15.md");
        assert_eq!(month_path(2026, 1), "Nhật ký/2026/01/2026-01.md");
        assert_eq!(year_path(2026), "Nhật ký/2026/2026.md");
        assert_eq!(project_path("du-an-a"), "Dự án/du-an-a.md");
        assert_eq!(weekday_vi(d), "Thứ Năm");
        for p in [day_path(d), project_path("x"), MARKER.to_string()] {
            assert!(p.nfc().eq(p.chars()), "{p} không ở dạng NFC");
        }
    }
}
