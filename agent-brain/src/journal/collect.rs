//! brain.db → `Model`, trong một giao dịch đọc (D-04, D-06, D-12, D-23).
//!
//! Chỉ đọc các cột allowlist: `projects` (id, slug, name_vi qua `list_project_display_map`),
//! `tasks` (id, project_id, slug, title, status, approved_at), `proposals.title`, và các run của
//! task (vai, trạng thái, outcome, thời điểm, vòng). Không đọc đường dẫn, nhánh, thư mục chạy,
//! tệp plan hay báo cáo (D-03).

use super::config::{journal_group_of, JournalConfig};
use super::model::{AttentionKind, DayStatus, Model, ProjectRef, StatusKind, TaskDay};
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

fn slug_ok(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|&c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
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
}

fn day_status(last: &RunRow, is_last_overall: bool, task_status: &str) -> DayStatus {
    let (key, _) = crate::dashboard::taskstate::step_state(Some(last));
    let task_done = is_last_overall && crate::tasks::SUCCESS_STATUSES.contains(&task_status);
    let attention = |k: AttentionKind| DayStatus {
        kind: StatusKind::Attention(k),
        label: k.label().to_string(),
    };
    match key {
        "xong" => DayStatus {
            kind: StatusKind::Done,
            label: if matches!(last.role.as_str(), "analyst" | "analysis-review") {
                "đã trả lời".to_string()
            } else {
                "xong".to_string()
            },
        },
        _ if task_done && last.status != "running" => DayStatus {
            kind: StatusKind::Done,
            label: if matches!(last.role.as_str(), "analyst" | "analysis-review") {
                "đã trả lời".to_string()
            } else {
                "xong".to_string()
            },
        },
        "het_quota" => attention(AttentionKind::Quota),
        "moi_truong" => attention(AttentionKind::EnvDown),
        "can_sua" => attention(AttentionKind::QaFix),
        "vuong_mac" => attention(AttentionKind::Blocked),
        "vi_pham" => attention(AttentionKind::ProtocolViolation),
        "loi" if last.status == "timed_out" => attention(AttentionKind::Timeout),
        "loi" => attention(AttentionKind::Failed),
        "killed" => DayStatus {
            kind: StatusKind::Stopped,
            label: "đã dừng".to_string(),
        },
        "dang_chay" => DayStatus {
            kind: StatusKind::Running,
            label: "đang chạy".to_string(),
        },
        _ => DayStatus {
            kind: StatusKind::Active,
            label: "đang làm".to_string(),
        },
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
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

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

        for (day, idx) in by_day {
            let (_, day_end) = day_bounds_ms(day, tz);
            let last_i = *idx.last().expect("ngày có ít nhất một run");
            let last = &runs[last_i];
            let status = day_status(last, last_i == runs.len() - 1, &t.status);

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
            let first_run_ms = idx.iter().map(|&i| runs[i].started_at).min().unwrap_or(0);

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

    Ok(Model {
        today,
        days,
        attention: BTreeMap::new(),
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
    }
}
