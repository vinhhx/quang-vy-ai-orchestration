pub mod scheduler;
pub mod triage;

use crate::dashboard::AppState;
use crate::db::IntakeRow;
use crate::error::BrainResult;
use rusqlite::Connection;
use std::sync::Arc;

/// Lấy MỘT intake `queued` cũ nhất (ưu tiên `urgent` > `normal` > `low`, rồi `created_at ASC, id ASC`).
pub fn next_queued_intake(conn: &Connection) -> BrainResult<Option<IntakeRow>> {
    let sql = "SELECT id, source, project_id, request, priority, status, error, created_at, updated_at
               FROM intake
               WHERE status = 'queued'
               ORDER BY
                   CASE priority
                       WHEN 'urgent' THEN 1
                       WHEN 'normal' THEN 2
                       WHEN 'low' THEN 3
                       ELSE 4
                   END ASC,
                   created_at ASC,
                   id ASC
               LIMIT 1";
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        Ok(Some(IntakeRow {
            id: r.get(0)?,
            source: r.get(1)?,
            project_id: r.get(2)?,
            request: r.get(3)?,
            priority: r.get(4)?,
            status: r.get(5)?,
            error: r.get(6)?,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

/// Một nhịp xử lý của orchestrator: nếu bật `enabled`, lấy một intake queued và phân tích thành proposals.
pub fn tick(st: &AppState) -> BrainResult<()> {
    if !st.cfg.orchestrator.enabled {
        return Ok(());
    }

    let conn = crate::db::open(&st.db_path)?;
    let Some(intake) = next_queued_intake(&conn)? else {
        return Ok(());
    };

    crate::db::set_intake_status(&conn, intake.id, "triaging")?;

    match triage::triage_with_warnings(st, &intake) {
        Ok((valid_proposals, warnings)) => {
            if valid_proposals.is_empty() {
                let err_msg = if !warnings.is_empty() {
                    warnings.join("; ")
                } else {
                    "Không có đề xuất nào hợp lệ".to_string()
                };
                crate::db::set_intake_status_error(&conn, intake.id, "failed", Some(&err_msg))?;
            } else {
                // Đảm bảo projects trong DB được cập nhật từ projects_map.json
                if let Ok(map) = crate::projects::load_map(&st.map_path) {
                    let projs = crate::projects::projects_of(&map);
                    let _ = crate::db::sync_projects(&conn, &projs);
                }

                for draft in &valid_proposals {
                    let proj_row_id = crate::db::project_row_id(&conn, &draft.project)?;
                    let new_p = crate::db::NewProposal {
                        intake_id: intake.id,
                        project_id: proj_row_id,
                        title: &draft.title,
                        request: &draft.request,
                        priority: &draft.priority,
                        team: &draft.team,
                        workflow: &draft.workflow,
                        reason: draft.reason.as_deref(),
                    };
                    crate::db::insert_proposal(&conn, &new_p)?;
                }

                let warning_text = if !warnings.is_empty() {
                    Some(warnings.join("; "))
                } else {
                    None
                };
                crate::db::set_intake_status_error(
                    &conn,
                    intake.id,
                    "proposed",
                    warning_text.as_deref(),
                )?;
            }
        }
        Err(e) => {
            crate::db::set_intake_status_error(&conn, intake.id, "failed", Some(&e.message))?;
        }
    }

    Ok(())
}

/// Vòng lặp nền: gọi `tick` mỗi 5 giây, khởi động từ `dashboard::cmd_serve`.
/// Lỗi trong `tick` chỉ ghi log, không làm chết server.
pub fn spawn_loop(st: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown = st.shutdown.clone();
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    let st_clone = st.clone();
                    let res = tokio::task::spawn_blocking(move || {
                        let t_res = tick(&st_clone);
                        let s_res = scheduler::schedule_tick(&st_clone);
                        (t_res, s_res)
                    }).await;

                    match res {
                        Ok((t_res, s_res)) => {
                            if let Err(e) = t_res {
                                crate::term::emit(&format!("Orchestrator triage error: {}", e.message));
                            }
                            if let Err(e) = s_res {
                                crate::term::emit(&format!("Orchestrator scheduler error: {}", e.message));
                            }
                        }
                        Err(join_err) => {
                            crate::term::emit(&format!("Orchestrator thread error: {join_err}"));
                        }
                    }
                }
            }
        }
    })
}
