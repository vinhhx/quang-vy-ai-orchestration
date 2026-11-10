//! `Model` → nội dung tệp (đường dẫn tương đối → chữ), định dạng demo D-17. Thuần: không đọc
//! đồng hồ, không I/O; chữ từ brain.db chỉ đến đây dưới dạng `SafeText`.

use super::collect::ROLE_LABELS;
use super::config::JournalConfig;
use super::model::{
    day_path, month_path, project_path, weekday_vi, year_path, AttentionItem, AttentionKind, Model,
    StatusKind, TaskDay, MARKER, TODAY_PATH,
};
use chrono::{Datelike, NaiveDate};
use std::collections::{BTreeMap, BTreeSet};
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

    // D-16: danh sách việc cần anh xử lý nằm ngay dưới tiêu đề, trước các nhóm.
    let mut items: Vec<&AttentionItem> = model
        .attention
        .get(&day)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    items.sort_by_key(|i| (i.kind.category(), i.day, i.task_id));
    if !items.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "## Cần anh xử lý ({})", items.len());
        for i in items {
            let _ = write!(
                out,
                "- **{}** — {} — {}",
                i.kind.label(),
                project_link(&i.project_slug),
                i.title.cut(cfg.callout_title_chars)
            );
            if i.day < day {
                let _ = write!(out, " · từ {}", i.day.format("%d/%m"));
            }
            out.push('\n');
        }
    }

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

/// Mỗi task một lần trong kỳ, ở ngày hoạt động cuối cùng của nó trong kỳ (`keep` chọn các mục
/// thuộc kỳ). Khóa = task_id.
fn last_per_task<'a>(
    model: &'a Model,
    keep: impl Fn(NaiveDate, &TaskDay) -> bool,
) -> BTreeMap<i64, (NaiveDate, &'a TaskDay)> {
    let mut out = BTreeMap::new();
    for (day, tasks) in &model.days {
        for t in tasks.iter().filter(|t| keep(*day, t)) {
            out.insert(t.task_id, (*day, t));
        }
    }
    out
}

/// (số task phân biệt, số task có trạng thái ở ngày cuối cùng là xong).
fn counts(period: &BTreeMap<i64, (NaiveDate, &TaskDay)>) -> (usize, usize) {
    let done = period
        .values()
        .filter(|(_, t)| t.status.kind == StatusKind::Done)
        .count();
    (period.len(), done)
}

/// Số task theo dự án, nhiều trước rồi theo tên.
fn by_project(period: &BTreeMap<i64, (NaiveDate, &TaskDay)>) -> Vec<(String, usize)> {
    let mut m: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, t) in period.values() {
        *m.entry(t.project.slug.as_str()).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = m.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

fn ym(d: NaiveDate) -> (i32, u32) {
    (d.year(), d.month())
}

fn project_link(slug: &str) -> String {
    format!("[[Dự án/{slug}|{slug}]]")
}

fn render_month(model: &Model, year: i32, month: u32, day_files: &BTreeSet<NaiveDate>) -> String {
    let period = last_per_task(model, |d, _| ym(d) == (year, month));
    let (n, done) = counts(&period);
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "month: {year:04}-{month:02}");
    out.push_str("tags: [nhat-ky-thang]\n---\n");
    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(out, "# Tháng {month:02}/{year:04} — {n} việc ({done} xong)");
    out.push('\n');
    out.push_str("| Nhóm | Số việc |\n|---|---|\n");
    for g in &model.groups {
        let c = period
            .values()
            .filter(|(_, t)| t.project.group_id == g.id)
            .count();
        if c > 0 {
            let _ = writeln!(out, "| {} | {c} |", g.label);
        }
    }
    out.push_str("\n## Theo dự án\n");
    for (slug, c) in by_project(&period) {
        let _ = writeln!(out, "- {} — {c} việc", project_link(&slug));
    }
    out.push_str("\n## Các ngày\n");
    for d in day_files.iter().filter(|d| ym(**d) == (year, month)) {
        let c = model.days.get(d).map_or(0, Vec::len);
        let _ = writeln!(out, "- [[{}]] — {c} việc", d.format("%Y-%m-%d"));
    }
    out.push_str("\n---\n");
    let _ = writeln!(out, "Năm: [[{year:04}]]");
    out
}

fn render_year(model: &Model, year: i32, months: &BTreeSet<(i32, u32)>) -> String {
    let period = last_per_task(model, |d, _| d.year() == year);
    let (n, _) = counts(&period);
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "year: {year:04}");
    out.push_str("tags: [nhat-ky-nam]\n---\n");
    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(out, "# Năm {year:04} — {n} việc");
    out.push('\n');
    out.push_str("| Tháng | Số việc | Dự án chính |\n|---|---|---|\n");
    for &(y, m) in months.iter().filter(|(y, _)| *y == year) {
        let p = last_per_task(model, |d, _| ym(d) == (y, m));
        let top: Vec<String> = by_project(&p).into_iter().take(3).map(|(s, _)| s).collect();
        let _ = writeln!(
            out,
            "| [[{y:04}-{m:02}]] | {} | {} |",
            p.len(),
            top.join(", ")
        );
    }
    out
}

fn render_project(model: &Model, cfg: &JournalConfig, slug: &str) -> String {
    let all = last_per_task(model, |_, t| t.project.slug == slug);
    let (n, done) = counts(&all);
    let mut months: BTreeSet<(i32, u32)> = BTreeSet::new();
    for (day, tasks) in &model.days {
        if tasks.iter().any(|t| t.project.slug == slug) {
            months.insert(ym(*day));
        }
    }
    let mut out = String::new();
    out.push_str("---\ntags: [du-an]\n---\n");
    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(out, "# {slug} — {n} việc ({done} xong)");
    for (i, &(y, m)) in months.iter().rev().enumerate() {
        out.push('\n');
        let _ = writeln!(out, "## [[{y:04}-{m:02}]]");
        let p = last_per_task(model, |d, t| t.project.slug == slug && ym(d) == (y, m));
        if i < cfg.project_page_months as usize {
            let mut rows: Vec<(NaiveDate, i64, &TaskDay)> =
                p.iter().map(|(id, (d, t))| (*d, *id, *t)).collect();
            rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            for (d, _, t) in rows {
                let (_, icon) = callout_kind(t.status.kind);
                let _ = writeln!(
                    out,
                    "- [[{}]] {icon} {}",
                    d.format("%Y-%m-%d"),
                    t.title.cut(cfg.callout_title_chars)
                );
            }
        } else {
            let _ = writeln!(out, "- {} việc", p.len());
        }
    }
    out
}

fn render_today(
    model: &Model,
    day_files: &BTreeSet<NaiveDate>,
    months: &BTreeSet<(i32, u32)>,
) -> String {
    let today = model.today;
    let mut out = String::new();
    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(
        out,
        "# Hôm nay — {} {}",
        weekday_vi(today),
        today.format("%d/%m/%Y")
    );
    out.push('\n');
    if day_files.contains(&today) {
        let _ = writeln!(out, "![[{}]]", today.format("%Y-%m-%d"));
    } else {
        out.push_str("Hôm nay chưa có việc nào.\n");
    }
    out.push_str("\n---\n");
    let mut parts: Vec<String> = Vec::new();
    if let Some(prev) = day_files.iter().rev().find(|d| **d < today) {
        // IN-02: ngày trước đó có việc không nhất thiết là hôm qua.
        let label = if today.pred_opt() == Some(*prev) {
            "Hôm qua"
        } else {
            "Trước đó"
        };
        parts.push(format!("{label}: [[{}]]", prev.format("%Y-%m-%d")));
    }
    if months.contains(&ym(today)) {
        parts.push(format!(
            "Tháng: [[{:04}-{:02}]]",
            today.year(),
            today.month()
        ));
    }
    if months.iter().any(|(y, _)| *y == today.year()) {
        parts.push(format!("Năm: [[{:04}]]", today.year()));
    }
    let mut blocks: Vec<String> = Vec::new();
    if !parts.is_empty() {
        blocks.push(parts.join(" · "));
    }
    if !model.project_slugs.is_empty() {
        let links: Vec<String> = model
            .project_slugs
            .iter()
            .map(|s| project_link(s))
            .collect();
        blocks.push(format!("Dự án: {}", links.join(" · ")));
    }
    out.push_str(&blocks.join("\n\n"));
    if !blocks.is_empty() {
        out.push('\n');
    }
    out
}

/// Mọi tệp của vault (D-02/D-17): Today, ngày, tháng, năm, dự án. Chỉ liên kết tới tệp có mặt
/// trong chính tập kết quả (liên kết treo sẽ tạo ghi chú rỗng trên điện thoại).
pub fn render_all(model: &Model, cfg: &JournalConfig) -> BTreeMap<String, String> {
    let mut day_files: BTreeSet<NaiveDate> = model
        .days
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(d, _)| *d)
        .collect();
    if model
        .attention
        .get(&model.today)
        .is_some_and(|v| !v.is_empty())
    {
        day_files.insert(model.today);
    }
    let days: Vec<NaiveDate> = day_files.iter().copied().collect();
    let months: BTreeSet<(i32, u32)> = days.iter().map(|d| ym(*d)).collect();
    let years: BTreeSet<i32> = months.iter().map(|(y, _)| *y).collect();

    let mut files = BTreeMap::new();
    for (i, day) in days.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| days[j]);
        let next = days.get(i + 1).copied();
        let tasks: &[TaskDay] = model.days.get(day).map(Vec::as_slice).unwrap_or(&[]);
        let text = render_day(*day, tasks, model, cfg, prev, next);
        files.insert(day_path(*day), text);
    }
    for &(y, m) in &months {
        files.insert(month_path(y, m), render_month(model, y, m, &day_files));
    }
    for &y in &years {
        files.insert(year_path(y), render_year(model, y, &months));
    }
    for slug in &model.project_slugs {
        files.insert(project_path(slug), render_project(model, cfg, slug));
    }
    files.insert(
        TODAY_PATH.to_string(),
        render_today(model, &day_files, &months),
    );
    files
}

#[cfg(test)]
mod tests {
    use super::super::model::{DayStatus, ProjectRef};
    use super::super::sanitize::sanitize_text;
    use super::*;

    fn task(id: i64, d: NaiveDate) -> TaskDay {
        TaskDay {
            task_id: id,
            day: d,
            project: ProjectRef {
                slug: "p".to_string(),
                group_id: "cong-viec".to_string(),
                group_label: "Công việc".to_string(),
            },
            title: sanitize_text(&format!("Việc số {id}"), 140, &[]),
            request: sanitize_text("", 140, &[]),
            status: DayStatus {
                kind: StatusKind::Done,
                label: "xong".to_string(),
            },
            runs: 1,
            rounds: 1,
            roles: vec!["dev".to_string()],
            duration_ms: 60_000,
            first_run_ms: id,
        }
    }

    #[test]
    fn project_page_folds_old_months() {
        let cfg = JournalConfig::default();
        assert_eq!(cfg.project_page_months, 3);
        let mut days = BTreeMap::new();
        for (i, (y, m)) in [(2025, 9), (2025, 10), (2025, 11), (2025, 12), (2026, 1)]
            .iter()
            .enumerate()
        {
            let d = NaiveDate::from_ymd_opt(*y, *m, 5).unwrap();
            days.insert(d, vec![task(i as i64 + 1, d), task(i as i64 + 101, d)]);
        }
        let model = Model {
            today: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            days,
            attention: BTreeMap::new(),
            groups: cfg.groups.clone(),
            project_slugs: vec!["p".to_string()],
        };
        let text = render_project(&model, &cfg, "p");
        let lines: Vec<&str> = text.lines().collect();
        for full in ["## [[2026-01]]", "## [[2025-12]]", "## [[2025-11]]"] {
            assert!(lines.contains(&full), "{full}\n{text}");
        }
        assert!(lines.contains(&"- [[2026-01-05]] ✓ Việc số 5"), "{text}");
        assert!(lines.contains(&"- [[2025-11-05]] ✓ Việc số 3"), "{text}");
        // The two older months collapse into a count line.
        for old in ["## [[2025-10]]", "## [[2025-09]]"] {
            let at = lines.iter().position(|l| *l == old).expect(old);
            assert_eq!(lines[at + 1], "- 2 việc", "{text}");
        }
        assert!(!text.contains("[[2025-10-05]]"), "{text}");
        assert!(text.contains("# p — 10 việc (10 xong)"), "{text}");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(0), "");
        assert_eq!(fmt_duration(59_999), "< 1 phút");
        assert_eq!(fmt_duration(34 * 60_000), "34 phút");
        assert_eq!(fmt_duration(120 * 60_000), "2 giờ");
        assert_eq!(fmt_duration(125 * 60_000 + 10), "2 giờ 5 phút");
    }
}
