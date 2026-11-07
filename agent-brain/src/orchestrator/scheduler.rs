//! Bộ lập lịch điều phối (Scheduler) cho orchestrator:
//! - Duyệt proposal approved theo thứ tự ưu tiên (urgent > normal > low), tối đa max_parallel.
//! - Task hết quota không tính vào slot.
//! - Quét task dừng vì quota ghi vào bảng quota_stops (cột notified giữ nguyên 0, thông báo Telegram do telegram/detect.rs đảm nhận tự động).
//! - Tự động tiếp tục (auto_resume) khi auto_resume_quota = true và đã qua resume_after_minutes (chỉ một lần mỗi lần dừng).
//! - Không tự đổi engine.

use crate::dashboard::api_tasks::{start_new_task, Origin, StartOutcome, StartRequest};
use crate::dashboard::AppState;
use crate::error::BrainResult;

/// Tính số task đang chạy trong hệ thống: bao gồm các task có run hoặc auto_state đang 'running' trong CSDL,
/// và các job đang chạy trong JobManager (không tính trùng).
pub(crate) fn active_task_count(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<u32> {
    let mut active_task_ids = std::collections::HashSet::new();
    let mut stmt = conn.prepare(
        "SELECT task_id FROM runs WHERE status = 'running' AND task_id IS NOT NULL
         UNION
         SELECT id FROM tasks WHERE auto_state = 'running'",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    for id in rows {
        active_task_ids.insert(id?);
    }

    let mut unassociated_jobs = 0;
    for job in st.jobs.list() {
        if job.state == "running" {
            if let Some(tid) = job.task_id {
                active_task_ids.insert(tid);
            } else {
                unassociated_jobs += 1;
            }
        }
    }

    Ok((active_task_ids.len() + unassociated_jobs) as u32)
}

/// Nhịp xử lý của bộ lập lịch: gọi định kỳ bởi orchestrator loop
pub fn schedule_tick(st: &AppState) -> BrainResult<()> {
    let orch = st.orch();
    if !orch.enabled {
        return Ok(());
    }

    let conn = crate::db::open(&st.db_path)?;
    let max_parallel = orch.max_parallel;

    // (b) & (c): Quét các task dừng vì quota để ghi vào quota_stops và xử lý auto_resume nếu có
    handle_quota_stops(st, &conn)?;

    // (a): Bắt đầu các proposal approved theo thứ tự ưu tiên
    let mut active = active_task_count(st, &conn)?;
    let approved_props = crate::db::list_approved_proposals(&conn)?;

    for prop in approved_props {
        if active >= max_parallel {
            break;
        }

        let project_slug: String = match conn.query_row(
            "SELECT slug FROM projects WHERE id = ?1",
            rusqlite::params![prop.project_id],
            |r| r.get(0),
        ) {
            Ok(s) => s,
            Err(_) => {
                crate::db::set_proposal_failed(&conn, prop.id, "Dự án không tồn tại trong CSDL")?;
                continue;
            }
        };

        let title_slug = crate::routing::slugify(&prop.title);
        let name_opt = if title_slug.is_empty() {
            None
        } else {
            Some(title_slug.as_str())
        };

        let req = StartRequest {
            project: &project_slug,
            request: &prop.request,
            name: name_opt,
            workflow: Some(&prop.workflow),
            team: Some(&prop.team),
            origin: Origin::Dashboard,
            dry_run: false,
        };

        match start_new_task(st, &req)? {
            StartOutcome::Started { task_id, .. } => {
                active += 1;
                let _ = crate::db::set_task_level(&conn, task_id, &prop.level);
                crate::db::set_proposal_started(&conn, prop.id, Some(task_id))?;
            }
            StartOutcome::PreflightRefused { reason, kind } => {
                if kind == "busy" {
                    // Dự án đang bận (có job chạy): tạm thời, giữ approved và thử proposal tiếp theo
                    continue;
                }
                crate::db::set_proposal_failed(&conn, prop.id, &reason)?;
            }
            StartOutcome::Busy(_) => {
                // Tạm thời bận, giữ approved và thử proposal tiếp theo
                continue;
            }
            StartOutcome::Invalid(reason) => {
                crate::db::set_proposal_failed(&conn, prop.id, &reason)?;
            }
            StartOutcome::ProjectRefused { body, .. } => {
                let err_str = body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Dự án bị từ chối");
                crate::db::set_proposal_failed(&conn, prop.id, err_str)?;
            }
            StartOutcome::Unavailable => {
                break;
            }
            StartOutcome::Ready(_) => {
                break;
            }
        }
    }

    Ok(())
}

fn handle_quota_stops(st: &AppState, conn: &rusqlite::Connection) -> BrainResult<()> {
    let now = crate::db::now_ms();

    // Quét các task dừng vì quota: auto_stop_kind = 'het_quota' hoặc run cuối có outcome = 'quota'
    // Lưu ý: Chỉ xét task không đang chạy (auto_state != 'running' hoặc NULL)
    let mut stmt = conn.prepare(
        "SELECT t.id, t.auto_stop_at, COALESCE(t.auto_stop_kind, ''), t.project_id, t.branch, t.slug,
                p.slug AS project_slug, p.profile AS project_profile,
                r.engine, r.ended_at, r.outcome
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         LEFT JOIN runs r ON r.id = (
             SELECT id FROM runs WHERE task_id = t.id ORDER BY started_at DESC, id DESC LIMIT 1
         )
         WHERE (t.auto_stop_kind = 'het_quota' OR r.outcome = 'quota')
           AND (t.auto_state IS NULL OR t.auto_state != 'running')
           AND EXISTS (
               SELECT 1 FROM proposals pr WHERE pr.task_id = t.id AND pr.status = 'started'
           )",
    )?;

    struct QuotaCandidate {
        task_id: i64,
        stopped_at: i64,
        engine: String,
        project_slug: String,
        project_profile: String,
        branch: String,
    }

    let candidates: Vec<QuotaCandidate> = stmt
        .query_map([], |row| {
            let task_id: i64 = row.get(0)?;
            let auto_stop_at: Option<i64> = row.get(1)?;
            let project_slug: String = row.get(6)?;
            let project_profile: String = row.get(7)?;
            let branch: String = row.get(4)?;
            let run_engine: Option<String> = row.get(8)?;
            let run_ended_at: Option<i64> = row.get(9)?;

            let engine = run_engine.unwrap_or_else(|| "claude".to_string());
            let stopped_at = auto_stop_at
                .filter(|&t| t > 0)
                .or_else(|| run_ended_at.filter(|&t| t > 0))
                .unwrap_or(now);

            Ok(QuotaCandidate {
                task_id,
                stopped_at,
                engine,
                project_slug,
                project_profile,
                branch,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // (b) Ghi quota_stops khi phát hiện dừng lần đầu.
    // Cột notified để nguyên 0: thông báo Telegram do telegram/detect.rs đảm nhận tự động.
    for cand in &candidates {
        crate::db::insert_quota_stop(conn, cand.task_id, cand.stopped_at, &cand.engine)?;
    }

    // (c) auto_resume chỉ khi auto_resume_quota = true và đã qua resume_after_minutes
    let orch = st.orch();
    if orch.auto_resume_quota {
        let resume_after_ms = orch.resume_after_minutes as i64 * 60 * 1000;
        let quota_stops = crate::db::list_quota_stops(conn)?;

        for qs in quota_stops {
            if now >= qs.stopped_at + resume_after_ms {
                // Giới hạn auto_resume chỉ cho task có trong proposals (proposals.task_id = tasks.id và proposals.status = started)
                let has_started_proposal: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM proposals WHERE task_id = ?1 AND status = 'started')",
                        rusqlite::params![qs.task_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if !has_started_proposal {
                    let _ = crate::db::delete_quota_stop(conn, qs.task_id);
                    continue;
                }

                // Kiểm tra xem task có đang chạy job không
                let already_running = st
                    .jobs
                    .list()
                    .iter()
                    .any(|j| j.task_id == Some(qs.task_id) && j.state == "running");
                if already_running {
                    continue;
                }

                let cand_opt = candidates.iter().find(|c| c.task_id == qs.task_id);
                let (proj_slug, proj_prof, branch) = if let Some(c) = cand_opt {
                    (
                        c.project_slug.clone(),
                        c.project_profile.clone(),
                        c.branch.clone(),
                    )
                } else {
                    let info: Option<(String, String, String)> = conn
                        .query_row(
                            "SELECT p.slug, p.profile, t.branch FROM tasks t JOIN projects p ON p.id = t.project_id WHERE t.id = ?1",
                            rusqlite::params![qs.task_id],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                        )
                        .ok();
                    match info {
                        Some(i) => i,
                        None => continue,
                    }
                };

                let spec = crate::dashboard::jobs::JobSpec {
                    role: "auto".to_string(),
                    project: proj_slug,
                    profile: proj_prof,
                    branch: Some(branch),
                    task_id: Some(qs.task_id),
                    args: vec!["auto".to_string(), qs.task_id.to_string()],
                };

                // Chạy tiếp qua đường autopilot hiện có; xóa khỏi quota_stops để chỉ chạy 1 lần mỗi lần dừng
                match st.jobs.start(spec) {
                    Ok(_) => {
                        crate::db::delete_quota_stop(conn, qs.task_id)?;
                        let _ = conn.execute(
                            "UPDATE tasks SET auto_stop_kind = NULL WHERE id = ?1",
                            rusqlite::params![qs.task_id],
                        );
                    }
                    Err(e) => {
                        crate::term::emit(&format!(
                            "Lỗi auto_resume task {}: {}",
                            qs.task_id, e.message
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}
