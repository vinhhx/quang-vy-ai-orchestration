//! `Model` → nội dung tệp (đường dẫn tương đối → chữ), định dạng demo D-17. Thuần: không đọc
//! đồng hồ, không I/O; chữ từ brain.db chỉ đến đây dưới dạng `SafeText`.

use super::collect::ROLE_LABELS;
use super::config::JournalConfig;
use super::model::{day_path, weekday_vi, AttentionKind, Model, StatusKind, TaskDay, MARKER};
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// `< 1 phút`, `{m} phút`, `{h} giờ`, `{h} giờ {m} phút`; chuỗi rỗng khi `ms <= 0`.
pub fn fmt_duration(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    if ms < 60_000 {
        return "< 1 phút".to_string();
    }
    let minutes = ms / 60_000;
    if minutes < 60 {
        return format!("{minutes} phút");
    }
    let (h, m) = (minutes / 60, minutes % 60);
    if m == 0 {
        format!("{h} giờ")
    } else {
        format!("{h} giờ {m} phút")
    }
}

fn callout_kind(kind: StatusKind) -> (&'static str, &'static str) {
    match kind {
        StatusKind::Done => ("success", "✓"),
        StatusKind::Attention(
            AttentionKind::Failed | AttentionKind::Timeout | AttentionKind::ProtocolViolation,
        ) => ("failure", "✗"),
        StatusKind::Attention(_) => ("warning", "!"),
        StatusKind::Running | StatusKind::Active => ("info", "…"),
        StatusKind::Stopped => ("note", "■"),
    }
}

fn role_label(role: &str) -> &str {
    ROLE_LABELS
        .iter()
        .find(|(r, _)| *r == role)
        .map(|(_, l)| *l)
        .unwrap_or(role)
}

fn render_callout(out: &mut String, t: &TaskDay, cfg: &JournalConfig) {
    let (ty, icon) = callout_kind(t.status.kind);
    let slug = &t.project.slug;
    let _ = writeln!(
        out,
        "> [!{ty}]- {icon} {slug} — {}",
        t.title.cut(cfg.callout_title_chars)
    );
    let dur = fmt_duration(t.duration_ms);
    if dur.is_empty() {
        let _ = writeln!(out, "> **Trạng thái:** {}", t.status.label);
    } else {
        let _ = writeln!(out, "> **Trạng thái:** {} · {dur}", t.status.label);
    }
    let _ = writeln!(
        out,
        "> **Dự án:** [[Dự án/{slug}|{slug}]] · **Nhóm:** {} · **Số bước chạy:** {} · **Vòng:** {}",
        t.project.group_label, t.runs, t.rounds
    );
    let roles: Vec<&str> = t.roles.iter().map(|r| role_label(r)).collect();
    let _ = writeln!(out, "> **Vai đã chạy:** {}", roles.join(", "));
    if !t.request.is_empty() {
        out.push_str(">\n");
        let _ = writeln!(out, "> {}", t.request);
    }
}

fn render_day(
    day: NaiveDate,
    tasks: &[TaskDay],
    model: &Model,
    cfg: &JournalConfig,
    prev: Option<NaiveDate>,
    next: Option<NaiveDate>,
) -> String {
    let done = tasks
        .iter()
        .filter(|t| t.status.kind == StatusKind::Done)
        .count();
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "date: {}", day.format("%Y-%m-%d"));
    let _ = writeln!(out, "tasks: {}", tasks.len());
    out.push_str("tags: [nhat-ky]\n");
    out.push_str("---\n");
    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(
        out,
        "# {} {} — {} việc ({} xong)",
        weekday_vi(day),
        day.format("%d/%m/%Y"),
        tasks.len(),
        done
    );

    for g in &model.groups {
        let mut in_group: Vec<&TaskDay> = tasks
            .iter()
            .filter(|t| t.project.group_id == g.id)
            .collect();
        if in_group.is_empty() {
            continue;
        }
        in_group.sort_by_key(|t| (t.first_run_ms, t.task_id));
        out.push('\n');
        let _ = writeln!(out, "## {} ({})", g.label, in_group.len());
        for (i, t) in in_group.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            render_callout(&mut out, t, cfg);
        }
    }

    out.push_str("\n---\n");
    let mut footer = String::new();
    if let Some(p) = prev {
        let _ = write!(footer, "← [[{}]] · ", p.format("%Y-%m-%d"));
    }
    let _ = write!(footer, "Tháng: [[{:04}-{:02}]]", day.year(), day.month());
    if let Some(n) = next {
        let _ = write!(footer, " · [[{}]] →", n.format("%Y-%m-%d"));
    }
    out.push_str(&footer);
    out.push('\n');
    out
}

/// Mọi tệp ngày của mô hình (Today/tháng/năm/dự án do 07-03 thêm).
pub fn render_all(model: &Model, cfg: &JournalConfig) -> BTreeMap<String, String> {
    let days: Vec<NaiveDate> = model
        .days
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(d, _)| *d)
        .collect();
    let mut files = BTreeMap::new();
    for (i, day) in days.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| days[j]);
        let next = days.get(i + 1).copied();
        let text = render_day(*day, &model.days[day], model, cfg, prev, next);
        files.insert(day_path(*day), text);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(0), "");
        assert_eq!(fmt_duration(59_999), "< 1 phút");
        assert_eq!(fmt_duration(34 * 60_000), "34 phút");
        assert_eq!(fmt_duration(120 * 60_000), "2 giờ");
        assert_eq!(fmt_duration(125 * 60_000 + 10), "2 giờ 5 phút");
    }
}
