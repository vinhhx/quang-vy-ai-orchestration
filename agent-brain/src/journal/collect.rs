//! brain.db → `Model`, trong một giao dịch đọc (D-04, D-06, D-12, D-23).
//!
//! Chỉ đọc các cột allowlist: `projects` (id, slug, name_vi qua `list_project_display_map`),
//! `tasks` (id, project_id, slug, title, status, approved_at), `proposals.title`, và các run của
//! task (vai, trạng thái, outcome, thời điểm, vòng). Không đọc đường dẫn, nhánh, thư mục chạy,
//! tệp plan hay báo cáo (D-03).

use super::config::{journal_group_of, JournalConfig};
use super::model::{
    AttentionItem, AttentionKind, DayStatus, Model, ProjectRef, StatusKind, TaskDay,
};
use super::sanitize::sanitize_text;
use crate::db::RunRow;
use crate::error::BrainResult;
use chrono::{NaiveDate, TimeZone};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Thứ tự chuẩn của các vai và nhãn hiển thị.
pub const ROLE_LABELS: &[(&str, &str)] = &[
    ("lead", "Lead"),
    ("plan-review", "Review plan"),
    ("dev", "Dev"),
    ("code-review", "Review code"),
    ("qa", "QA"),
    ("analyst", "Phân tích"),
    ("analysis-review", "Review phân tích"),
    ("inspect", "Inspect"),
];

fn local_midnight_ms<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> i64 {
    (0..=3)
        .filter_map(|h| d.and_hms_opt(h, 0, 0))
        .find_map(|t| tz.from_local_datetime(&t).earliest())
        .map(|t| t.timestamp_millis())
        .unwrap_or_else(|| {
            d.and_hms_opt(0, 0, 0)
                .map(|t| t.and_utc().timestamp_millis())
                .unwrap_or(0)
        })
}

/// [đầu ngày, đầu ngày hôm sau) của ngày địa phương `d`, theo epoch ms.
pub fn day_bounds_ms<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> (i64, i64) {
    let next = d.succ_opt().unwrap_or(d);
    (local_midnight_ms(d, tz), local_midnight_ms(next, tz))
}

/// Ngày địa phương của thời điểm `ms`.
pub fn local_day<Tz: TimeZone>(ms: i64, tz: &Tz) -> NaiveDate {
    match tz.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.naive_local().date(),
        None => chrono::DateTime::from_timestamp_millis(ms)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH)
            .date_naive(),
    }
}

/// `^[a-z0-9][a-z0-9._-]{0,63}$`, không chứa `..` và không kết thúc bằng `.` (WR-02: `a..b`
/// hay `foo.` làm `check_rel` từ chối cả lần xuất).
fn slug_ok(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|&c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
        && !s.contains("..")
        && !s.ends_with('.')
}

/// Mốc mới nhất (epoch ms) trong brain.db mà `collect` dùng để xếp ngày: `runs.started_at`,
/// `runs.ended_at`, `tasks.approved_at`, `plan_approvals.approved_at`. `None` khi DB chưa có.
pub fn newest_activity_ms(conn: &Connection) -> BrainResult<Option<i64>> {
    Ok(conn.query_row(
        "SELECT max(m) FROM (
            SELECT max(started_at) AS m FROM runs
            UNION ALL SELECT max(ended_at) FROM runs
            UNION ALL SELECT max(approved_at) FROM tasks
            UNION ALL SELECT max(approved_at) FROM plan_approvals)",
        [],
        |r| r.get(0),
    )?)
}

/// Tên tệp của trang dự án: slug nếu khớp `^[a-z0-9][a-z0-9._-]{0,63}$`, nếu không thì
/// `slugify(name_vi)`, nếu rỗng hoặc trùng thì `du-an-{row_id}` (Pitfall 6).
pub fn project_file_key(
    slug: &str,
    name_vi: &str,
    row_id: i64,
    taken: &mut HashSet<String>,
) -> String {
    let mut key = if slug_ok(slug) {
        slug.to_string()
    } else {
        let s = crate::routing::slugify(name_vi);
        if slug_ok(&s) {
            s
        } else {
            String::new()
        }
    };
    if key.is_empty() || taken.contains(&key) {
        key = format!("du-an-{row_id}");
    }
    taken.insert(key.clone());
    key
}

struct TaskFacts {
    id: i64,
    project_id: i64,
    slug: String,
    request: String,
    status: String,
    approved_at: Option<i64>,
}

/// Các sự kiện của run cuối cùng (L) mà `classify_as_of` cần.
#[derive(Debug, Clone, Copy)]
pub struct LastRun<'a> {
    pub role: &'a str,
    pub status: &'a str,
    pub outcome: Option<&'a str>,
    pub started_at: i64,
}

impl<'a> LastRun<'a> {
    pub fn from_row(r: &'a RunRow) -> Self {
        LastRun {
            role: r.role.as_str(),
            status: r.status.as_str(),
            outcome: r.outcome.as_deref(),
            started_at: r.started_at,
        }
    }
}

/// Trạng thái của một task tại cuối ngày (D-16, RESEARCH §1): phân loại run cuối `last` (L) theo
/// đúng thứ tự ưu tiên. Thuần: chỉ nhận các sự kiện của run, mốc duyệt plan (`approvals`, epoch
/// ms), trạng thái task, L có phải run cuối cùng của task không, và thời điểm kết thúc ngày.
pub fn classify_as_of(
    last: &LastRun<'_>,
    approvals: &[i64],
    task_status: &str,
    is_last_overall: bool,
    day_end_ms: i64,
) -> DayStatus {
    let attention = |k: AttentionKind| DayStatus {
        kind: StatusKind::Attention(k),
        label: k.label().to_string(),
    };
    let done = |label: &str| DayStatus {
        kind: StatusKind::Done,
        label: label.to_string(),
    };
    let outcome = last.outcome.unwrap_or("");
    if outcome == crate::quota::OUTCOME_QUOTA {
        return attention(AttentionKind::Quota);
    }
    if outcome == crate::preflight::OUTCOME_ENV_DOWN {
        return attention(AttentionKind::EnvDown);
    }
    if outcome == "protocol_violation" {
        return attention(AttentionKind::ProtocolViolation);
    }
    if matches!(outcome, "dev:blocked" | "agy:empty" | "dsh:empty") {
        return attention(AttentionKind::Blocked);
    }
    if outcome == "qa:fix" {
        return attention(AttentionKind::QaFix);
    }
    match last.status {
        "killed" => {
            return DayStatus {
                kind: StatusKind::Stopped,
                label: "đã dừng".to_string(),
            }
        }
        "running" => {
            return DayStatus {
                kind: StatusKind::Running,
                label: "đang chạy".to_string(),
            }
        }
        "timed_out" => return attention(AttentionKind::Timeout),
        "failed" | "lost" => return attention(AttentionKind::Failed),
        _ => {}
    }
    let succeeded = last.status == "succeeded";
    if outcome == "qa:pass" {
        return done("xong");
    }
    if succeeded
        && matches!(last.role, "analyst" | "analysis-review")
        && outcome != "analyst:invalid"
    {
        return done("đã trả lời");
    }
    if succeeded
        && matches!(last.role, "lead" | "plan-review")
        && matches!(outcome, "" | "review:approve" | "review:pass")
        && !approvals
            .iter()
            .any(|&a| a >= last.started_at && a < day_end_ms)
    {
        return attention(AttentionKind::PlanWaiting);
    }
    if is_last_overall && crate::tasks::SUCCESS_STATUSES.contains(&task_status) {
        return done("xong");
    }
    DayStatus {
        kind: StatusKind::Active,
        label: "đang làm".to_string(),
    }
}

/// Đọc brain.db trong một giao dịch đọc và dựng `Model` cho mọi ngày có run, tới `today`.
pub fn collect<Tz: TimeZone>(
    conn: &Connection,
    map: &Value,
    cfg: &JournalConfig,
    secrets: &[String],
    today: NaiveDate,
    tz: &Tz,
) -> BrainResult<Model> {
    let tx = conn.unchecked_transaction()?;

    let projects = crate::db::list_project_display_map(&tx)?;
    let mut ids: Vec<i64> = projects.keys().copied().collect();
    ids.sort_unstable();
    let mut taken = HashSet::new();
    let mut project_refs: HashMap<i64, ProjectRef> = HashMap::new();
    for id in ids {
        let (slug, name) = &projects[&id];
        let key = project_file_key(slug, name, id, &mut taken);
        let group = journal_group_of(map, slug, cfg);
        project_refs.insert(
            id,
            ProjectRef {
                slug: key,
                group_id: group.id.clone(),
                group_label: group.label.clone(),
            },
        );
    }

    let tasks: Vec<TaskFacts> = {
        let mut stmt = tx.prepare(
            "SELECT id, project_id, slug, title, status, approved_at FROM tasks ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(TaskFacts {
                id: r.get(0)?,
                project_id: r.get(1)?,
                slug: r.get(2)?,
                request: r.get(3)?,
                status: r.get(4)?,
                approved_at: r.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    // Mốc duyệt plan của từng task (D-16): `plan_approvals.approved_at` (cộng `tasks.approved_at`).
    let mut approvals: HashMap<i64, Vec<i64>> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT task_id, approved_at FROM plan_approvals ORDER BY task_id, approved_at",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (task_id, at) = row?;
            approvals.entry(task_id).or_default().push(at);
        }
    }

    let mut proposal_titles: HashMap<i64, String> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT task_id, title FROM proposals WHERE task_id IS NOT NULL ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (task_id, title) = row?;
            proposal_titles.insert(task_id, title);
        }
    }

    let mut days: BTreeMap<NaiveDate, Vec<TaskDay>> = BTreeMap::new();
    let mut used_projects: BTreeSet<String> = BTreeSet::new();
    let mut carry: Vec<AttentionItem> = Vec::new();

    for t in &tasks {
        let Some(project) = project_refs.get(&t.project_id) else {
            continue;
        };
        let runs = crate::db::runs_for_task(&tx, t.id)?;
        if runs.is_empty() {
            continue;
        }
        let facts: Vec<crate::autopilot::decide::RunFact> = runs
            .iter()
            .map(crate::autopilot::decide::RunFact::from_row)
            .collect();
        let rounds_of = crate::autopilot::decide::effective_rounds(&facts);

        let mut by_day: BTreeMap<NaiveDate, Vec<usize>> = BTreeMap::new();
        for (i, r) in runs.iter().enumerate() {
            let at = r.ended_at.unwrap_or(r.started_at);
            let d = local_day(at, tz);
            if d <= today {
                by_day.entry(d).or_default().push(i);
            }
        }

        let raw_title = proposal_titles
            .get(&t.id)
            .filter(|s| !s.trim().is_empty())
            .map(String::as_str)
            .or_else(|| Some(t.request.as_str()).filter(|s| !s.trim().is_empty()))
            .unwrap_or(t.slug.as_str());
        let mut title = sanitize_text(raw_title, cfg.title_max_chars, secrets);
        if title.is_empty() {
            title = sanitize_text(&t.slug, cfg.title_max_chars, secrets);
        }
        let request = sanitize_text(&t.request, cfg.title_max_chars, secrets);

        // Mốc duyệt plan là hoạt động (D-16): ngày có duyệt cũng là ngày của task.
        let mut appr: Vec<i64> = approvals.get(&t.id).cloned().unwrap_or_default();
        if let Some(a) = t.approved_at {
            appr.push(a);
        }
        appr.sort_unstable();
        appr.dedup();
        let mut appr_by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
        for &a in &appr {
            let d = local_day(a, tz);
            if d <= today {
                appr_by_day.entry(d).or_insert(a);
            }
        }
        let mut active_days: BTreeSet<NaiveDate> = by_day.keys().copied().collect();
        active_days.extend(appr_by_day.keys().copied());

        // L(D): run cuối của task bắt đầu trước hết ngày D.
        let last_before = |end_ms: i64| runs.iter().rposition(|r| r.started_at < end_ms);
        let status_at = |end_ms: i64| match last_before(end_ms) {
            Some(li) => classify_as_of(
                &LastRun::from_row(&runs[li]),
                &appr,
                &t.status,
                li == runs.len() - 1,
                end_ms,
            ),
            None => DayStatus {
                kind: StatusKind::Active,
                label: "đang làm".to_string(),
            },
        };

        if !active_days.is_empty() && !active_days.contains(&today) {
            let (_, today_end) = day_bounds_ms(today, tz);
            if let (Some(li), StatusKind::Attention(kind)) =
                (last_before(today_end), status_at(today_end).kind)
            {
                let at = runs[li].ended_at.unwrap_or(runs[li].started_at);
                carry.push(AttentionItem {
                    task_id: t.id,
                    project_slug: project.slug.clone(),
                    title: title.clone(),
                    kind,
                    day: local_day(at, tz).min(today),
                });
            }
        }

        for day in active_days {
            let idx: Vec<usize> = by_day.get(&day).cloned().unwrap_or_default();
            let (_, day_end) = day_bounds_ms(day, tz);
            let status = status_at(day_end);

            let rounds = runs
                .iter()
                .enumerate()
                .filter(|(_, r)| r.started_at < day_end)
                .map(|(i, _)| rounds_of[i])
                .max()
                .unwrap_or(1)
                .max(1);

            let day_roles: HashSet<&str> = idx.iter().map(|&i| runs[i].role.as_str()).collect();
            let roles: Vec<String> = ROLE_LABELS
                .iter()
                .filter(|(r, _)| day_roles.contains(r))
                .map(|(r, _)| r.to_string())
                .collect();

            let duration_ms: i64 = idx
                .iter()
                .filter_map(|&i| runs[i].ended_at.map(|e| (e - runs[i].started_at).max(0)))
                .sum();
            let first_run_ms = idx
                .iter()
                .map(|&i| runs[i].started_at)
                .min()
                .or_else(|| appr_by_day.get(&day).copied())
                .unwrap_or(0);

            used_projects.insert(project.slug.clone());
            days.entry(day).or_default().push(TaskDay {
                task_id: t.id,
                day,
                project: project.clone(),
                title: title.clone(),
                request: request.clone(),
                status,
                runs: idx.len() as u32,
                rounds: rounds.clamp(1, u32::MAX as i64) as u32,
                roles,
                duration_ms,
                first_run_ms,
            });
        }
    }

    tx.finish()?;

    // Mục cần xử lý của từng ngày = các task hoạt động trong ngày có trạng thái Attention;
    // riêng hôm nay thêm các việc cũ chưa giải quyết (carry-over, D-16).
    let mut attention: BTreeMap<NaiveDate, Vec<AttentionItem>> = BTreeMap::new();
    for (day, tasks) in &days {
        for t in tasks {
            if let StatusKind::Attention(kind) = t.status.kind {
                attention.entry(*day).or_default().push(AttentionItem {
                    task_id: t.task_id,
                    project_slug: t.project.slug.clone(),
                    title: t.title.clone(),
                    kind,
                    day: *day,
                });
            }
        }
    }
    if !carry.is_empty() {
        attention.entry(today).or_default().extend(carry);
    }
    for items in attention.values_mut() {
        items.sort_by_key(|i| (i.kind.category(), i.day, i.task_id));
    }

    Ok(Model {
        today,
        days,
        attention,
        groups: cfg.groups.clone(),
        project_slugs: used_projects.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn ms(tz: &FixedOffset, y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        tz.with_ymd_and_hms(y, mo, d, h, mi, s)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn day_boundary_is_local_midnight() {
        let tz = FixedOffset::east_opt(7 * 3600).unwrap();
        let d15 = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let d16 = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();
        let late = ms(&tz, 2026, 1, 15, 23, 59, 59);
        let early = ms(&tz, 2026, 1, 16, 0, 0, 1);
        assert_eq!(local_day(late, &tz), d15);
        assert_eq!(local_day(early, &tz), d16);
        let (start, end) = day_bounds_ms(d15, &tz);
        assert_eq!(start, ms(&tz, 2026, 1, 15, 0, 0, 0));
        assert_eq!(end, ms(&tz, 2026, 1, 16, 0, 0, 0));
        assert!(late >= start && late < end);
        assert!(early >= end);
    }

    fn kind_of(role: &str, status: &str, outcome: Option<&str>, approvals: &[i64]) -> StatusKind {
        let last = LastRun {
            role,
            status,
            outcome,
            started_at: 1_000,
        };
        classify_as_of(&last, approvals, "open", true, 10_000).kind
    }

    #[test]
    fn quota_precedes_failed_and_violation_precedes_gate() {
        assert_eq!(
            kind_of("dev", "failed", Some("quota"), &[]),
            StatusKind::Attention(AttentionKind::Quota)
        );
        assert_eq!(
            kind_of("lead", "succeeded", Some("protocol_violation"), &[]),
            StatusKind::Attention(AttentionKind::ProtocolViolation)
        );
        assert_eq!(
            kind_of("lead", "succeeded", None, &[]),
            StatusKind::Attention(AttentionKind::PlanWaiting)
        );
        // Duyệt sau khi run bắt đầu (trước hết ngày) → không còn chờ duyệt.
        assert_eq!(
            kind_of("lead", "succeeded", None, &[2_000]),
            StatusKind::Active
        );
        // Duyệt sau hết ngày thì chưa tính.
        assert_eq!(
            kind_of("lead", "succeeded", None, &[10_000]),
            StatusKind::Attention(AttentionKind::PlanWaiting)
        );
        // Run bị dừng không phải việc cần xử lý.
        assert_eq!(kind_of("dev", "killed", None, &[]), StatusKind::Stopped);
    }

    #[test]
    fn project_file_key_falls_back() {
        let mut taken = HashSet::new();
        assert_eq!(project_file_key("du-an-a", "", 1, &mut taken), "du-an-a");
        assert_eq!(
            project_file_key("../Hack", "Dự Án Bê", 2, &mut taken),
            "du-an-be"
        );
        assert_eq!(project_file_key("du-an-a", "x", 3, &mut taken), "du-an-3");
        assert_eq!(project_file_key("/", "///", 4, &mut taken), "du-an-4");
        // WR-02: `..` and a trailing `.` would make `check_rel` refuse the whole export.
        assert_eq!(project_file_key("a..b", "", 5, &mut taken), "du-an-5");
        assert_eq!(project_file_key("foo.", "", 6, &mut taken), "du-an-6");
        assert_eq!(project_file_key("v1.2", "", 7, &mut taken), "v1.2");
    }
}
