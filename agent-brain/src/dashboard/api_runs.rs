//! Runs API endpoints: status summary, raw event log, and Kill.

use crate::dashboard::sse::status_json;
use crate::dashboard::{blocking, json_error, AppState};
use crate::error::BrainResult;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct KillBody {}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/runs/{id}", get(get_run))
        .route("/api/runs/{id}/kill", post(post_kill_run))
        .route("/api/runs/{id}/events/{seq}/raw", get(get_raw_event))
}

async fn get_run(State(st): State<Arc<AppState>>, AxumPath(id): AxumPath<i64>) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let run = match crate::db::get_run(&conn, id)? {
            Some(r) => r,
            None => {
                return Ok((
                    StatusCode::NOT_FOUND,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": format!("Không tìm thấy run {id}")
                    })),
                )
                    .into_response());
            }
        };

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(status_json(&run)),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

/// Kết quả của Kill (dashboard và bot Telegram dùng chung, 05.3 D-02).
#[derive(Debug, Clone, PartialEq)]
pub enum KillOutcome {
    /// Run không có.
    NotFound(String),
    /// Từ chối: run không thuộc task, không còn chạy, hoặc `cmd_kill` báo lỗi.
    Refused(String),
    /// `cmd_kill` đã chạy; `ok: false` khi còn tiến trình sót.
    Done { ok: bool, message: String },
}

/// Kill một run của task, chỉ qua `runner::cmd_kill` (PID + thời điểm bắt đầu,
/// cây tiến trình đã ghi). Từ chối run không thuộc task nào và run không còn chạy.
pub fn kill_run(st: &AppState, run_id: i64) -> BrainResult<KillOutcome> {
    let id = run_id;
    let conn = crate::db::open(&st.db_path)?;
    let run = match crate::db::get_run(&conn, id)? {
        Some(r) => r,
        None => return Ok(KillOutcome::NotFound(format!("Không tìm thấy run {id}"))),
    };

    let task_id = match run.task_id {
        Some(tid) => tid,
        None => {
            return Ok(KillOutcome::Refused(format!(
                "Run {id} không thuộc task nào — không dừng được từ dashboard"
            )));
        }
    };

    if run.status != "running" {
        return Ok(KillOutcome::Refused(format!(
            "Run {id} không còn chạy (trạng thái {})",
            run.status
        )));
    }

    // `cmd_kill` dừng "run đang chạy của task": chỉ an toàn khi đó đúng là run đã xác nhận.
    // Task có nhiều run `running` (ví dụ dòng cũ của tiến trình đã chết) thì từ chối,
    // không đoán run nào — tránh dừng nhầm run và ghi sai "Đã dừng run {id}".
    let running: Vec<i64> = crate::db::runs_for_task(&conn, task_id)?
        .into_iter()
        .filter(|r| r.status == "running")
        .map(|r| r.id)
        .collect();
    if running != [id] {
        let others: Vec<String> = running
            .iter()
            .filter(|r| **r != id)
            .map(|r| r.to_string())
            .collect();
        return Ok(KillOutcome::Refused(format!(
            "Task {task_id} có nhiều run đang chạy (run {}) — không chắc dừng đúng run {id}, mở dashboard để xử lý",
            others.join(", ")
        )));
    }

    let mut ctx = crate::app::open_ctx_with(st.cfg.clone(), &st.map_path, &st.db_path)?;
    match crate::runner::cmd_kill(&mut ctx, task_id) {
        Ok(1) => Ok(KillOutcome::Done {
            ok: false,
            message: "CẢNH BÁO: còn tiến trình sót".to_string(),
        }),
        Ok(_) => Ok(KillOutcome::Done {
            ok: true,
            message: format!("Đã dừng run {id}"),
        }),
        Err(e) => Ok(KillOutcome::Refused(e.message)),
    }
}

async fn post_kill_run(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(_body): Json<KillBody>,
) -> Response {
    let res = blocking(move || kill_run(&st, id)).await;

    match res {
        Ok(KillOutcome::NotFound(msg)) => json_error(StatusCode::NOT_FOUND, &msg),
        Ok(KillOutcome::Refused(msg)) => json_error(StatusCode::CONFLICT, &msg),
        Ok(KillOutcome::Done { ok, message }) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(serde_json::json!({
                "ok": ok,
                "message": message
            })),
        )
            .into_response(),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn get_raw_event(
    State(st): State<Arc<AppState>>,
    AxumPath((run_id, seq)): AxumPath<(i64, i64)>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let mut stmt =
            conn.prepare("SELECT kind, raw_json FROM events WHERE run_id = ?1 AND seq = ?2")?;
        let mut rows = stmt.query(rusqlite::params![run_id, seq])?;
        if let Some(r) = rows.next()? {
            let ev: String = r.get(0)?;
            let raw_json: String = r.get(1)?;
            Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "run_id": run_id,
                    "seq": seq,
                    "ev": ev,
                    "raw_json": raw_json,
                })),
            )
                .into_response())
        } else {
            Ok((
                StatusCode::NOT_FOUND,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": format!("Không tìm thấy dòng log {seq} của run {run_id}")
                })),
            )
                .into_response())
        }
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}
