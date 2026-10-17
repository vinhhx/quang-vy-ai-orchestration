//! Runs API endpoints: status summary, raw event log, and Kill.

use crate::dashboard::sse::status_json;
use crate::dashboard::{blocking, json_error, AppState};
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

async fn post_kill_run(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(_body): Json<KillBody>,
) -> Response {
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

        let task_id = match run.task_id {
            Some(tid) => tid,
            None => {
                return Ok((
                    StatusCode::CONFLICT,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": format!("Run {id} không thuộc task nào — không dừng được từ dashboard")
                    })),
                )
                    .into_response());
            }
        };

        if run.status != "running" {
            return Ok((
                StatusCode::CONFLICT,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": format!("Run {id} không còn chạy (trạng thái {})", run.status)
                })),
            )
                .into_response());
        }

        let mut ctx = crate::app::open_ctx_with(st.cfg.clone(), &st.map_path, &st.db_path)?;
        match crate::runner::cmd_kill(&mut ctx, task_id) {
            Ok(0) => Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "ok": true,
                    "message": format!("Đã dừng run {id}")
                })),
            )
                .into_response()),
            Ok(1) => Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "ok": false,
                    "message": "CẢNH BÁO: còn tiến trình sót"
                })),
            )
                .into_response()),
            Ok(_) => Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "ok": true,
                    "message": format!("Đã dừng run {id}")
                })),
            )
                .into_response()),
            Err(e) => Ok((
                StatusCode::CONFLICT,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": e.message
                })),
            )
                .into_response()),
        }
    })
    .await;

    match res {
        Ok(resp) => resp,
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
