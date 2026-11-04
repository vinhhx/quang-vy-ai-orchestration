//! API hàng chờ việc (intake) và đề xuất (proposals) cho dashboard và điều phối.

use crate::dashboard::api_tasks::MAX_REQUEST_CHARS;
use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/intake", get(list_intake).post(create_intake))
        .route("/api/intake/{id}/cancel", post(cancel_intake))
        .route("/api/proposals", get(list_proposals))
        .route("/api/proposals/{id}/priority", post(post_proposal_priority))
        .route("/api/proposals/{id}/approve", post(post_proposal_approve))
        .route("/api/proposals/{id}/reject", post(post_proposal_reject))
        .route("/api/orchestrator/status", get(get_orchestrator_status))
}

fn json_response<T: Serialize>(status: StatusCode, data: T) -> Response {
    (
        status,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(data),
    )
        .into_response()
}

fn internal_error(action: &str, e: &dyn std::fmt::Display) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ khi {action}: {e}"));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("Lỗi nội bộ khi {action}"),
    )
}

#[derive(Deserialize)]
pub struct IntakeQuery {
    pub status: Option<String>,
}

async fn list_intake(
    State(st): State<Arc<AppState>>,
    Query(query): Query<IntakeQuery>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        crate::db::list_intake(&conn, query.status.as_deref())
    })
    .await;

    match res {
        Ok(items) => json_response(StatusCode::OK, items),
        Err(e) => internal_error("đọc danh sách intake", &e),
    }
}

#[derive(Deserialize)]
pub struct CreateIntakeBody {
    pub request: String,
    pub project: Option<serde_json::Value>,
    pub priority: Option<String>,
}

async fn create_intake(
    State(st): State<Arc<AppState>>,
    Json(body): Json<CreateIntakeBody>,
) -> Response {
    let trimmed = body.request.trim();
    if trimmed.is_empty() {
        return json_error(StatusCode::UNPROCESSABLE_ENTITY, "Yêu cầu không được để trống");
    }
    if trimmed.chars().count() > MAX_REQUEST_CHARS {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("Yêu cầu quá dài (tối đa {MAX_REQUEST_CHARS} ký tự)"),
        );
    }

    let priority = match body.priority.as_deref() {
        Some(p) if !p.is_empty() => {
            if !["urgent", "normal", "low"].contains(&p) {
                return json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Mức ưu tiên không hợp lệ (phải là urgent, normal, low)",
                );
            }
            p.to_string()
        }
        _ => "normal".to_string(),
    };

    let req_text = trimmed.to_string();
    let proj_val = body.project;

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let project_id = match proj_val {
            Some(serde_json::Value::Number(n)) => {
                let id = n.as_i64().ok_or_else(|| {
                    crate::error::BrainError::error("ID dự án không hợp lệ")
                })?;
                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM projects WHERE id = ?1",
                        rusqlite::params![id],
                        |r| r.get::<_, i64>(0),
                    )
                    .map(|c| c > 0)
                    .unwrap_or(false);
                if !exists {
                    return Err(crate::error::BrainError::new(
                        format!("Không tìm thấy dự án với ID {id}"),
                        crate::error::EXIT_ERROR,
                    ));
                }
                Some(id)
            }
            Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                let s_trim = s.trim();
                if let Ok(id) = s_trim.parse::<i64>() {
                    let exists: bool = conn
                        .query_row(
                            "SELECT COUNT(*) FROM projects WHERE id = ?1",
                            rusqlite::params![id],
                            |r| r.get::<_, i64>(0),
                        )
                        .map(|c| c > 0)
                        .unwrap_or(false);
                    if exists {
                        Some(id)
                    } else {
                        match crate::db::project_row_id(&conn, s_trim) {
                            Ok(pid) => Some(pid),
                            Err(_) => {
                                return Err(crate::error::BrainError::new(
                                    format!("Không tìm thấy dự án '{s_trim}'"),
                                    crate::error::EXIT_ERROR,
                                ))
                            }
                        }
                    }
                } else {
                    match crate::db::project_row_id(&conn, s_trim) {
                        Ok(pid) => Some(pid),
                        Err(_) => {
                            return Err(crate::error::BrainError::new(
                                format!("Không tìm thấy dự án '{s_trim}'"),
                                crate::error::EXIT_ERROR,
                            ))
                        }
                    }
                }
            }
            _ => None,
        };

        let id = crate::db::insert_intake(&conn, "ui", project_id, &req_text, &priority)?;
        Ok(id)
    })
    .await;

    match res {
        Ok(id) => json_response(StatusCode::CREATED, serde_json::json!({ "id": id })),
        Err(e) => {
            if e.message.contains("Không tìm thấy dự án") {
                json_error(StatusCode::UNPROCESSABLE_ENTITY, &e.message)
            } else {
                internal_error("tạo intake", &e)
            }
        }
    }
}

async fn cancel_intake(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let intake = match crate::db::get_intake(&conn, id)? {
            Some(row) => row,
            None => {
                return Ok(None);
            }
        };
        crate::db::set_intake_status(&conn, id, "cancelled")?;
        Ok(Some(intake))
    })
    .await;

    match res {
        Ok(Some(_)) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "cancelled" }),
        ),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            &format!("Không tìm thấy intake #{id}"),
        ),
        Err(e) => internal_error("hủy intake", &e),
    }
}

#[derive(Deserialize)]
pub struct ProposalQuery {
    pub status: Option<String>,
}

async fn list_proposals(
    State(st): State<Arc<AppState>>,
    Query(query): Query<ProposalQuery>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        crate::db::list_proposals(&conn, query.status.as_deref())
    })
    .await;

    match res {
        Ok(items) => json_response(StatusCode::OK, items),
        Err(e) => internal_error("đọc danh sách đề xuất", &e),
    }
}

#[derive(Deserialize)]
pub struct PriorityBody {
    pub priority: String,
}

async fn post_proposal_priority(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<PriorityBody>,
) -> Response {
    let priority = body.priority.trim();
    if !["urgent", "normal", "low"].contains(&priority) {
        return json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Mức ưu tiên không hợp lệ (phải là urgent, normal, low)",
        );
    }
    let priority_str = priority.to_string();

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let prop = match crate::db::get_proposal(&conn, id)? {
            Some(p) => p,
            None => return Ok(Err(StatusCode::NOT_FOUND)),
        };

        if prop.status != "pending" && prop.status != "approved" {
            return Ok(Err(StatusCode::CONFLICT));
        }

        crate::db::set_proposal_priority(&conn, id, &priority_str)?;
        Ok(Ok(priority_str))
    })
    .await;

    match res {
        Ok(Ok(p)) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "priority": p }),
        ),
        Ok(Err(StatusCode::NOT_FOUND)) => json_error(
            StatusCode::NOT_FOUND,
            &format!("Không tìm thấy đề xuất #{id}"),
        ),
        Ok(Err(StatusCode::CONFLICT)) => json_error(
            StatusCode::CONFLICT,
            "Chỉ có thể đổi mức ưu tiên khi đề xuất đang ở trạng thái pending hoặc approved",
        ),
        Ok(Err(_)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "Lỗi không xác định"),
        Err(e) => internal_error("đổi mức ưu tiên đề xuất", &e),
    }
}

async fn post_proposal_approve(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let prop = match crate::db::get_proposal(&conn, id)? {
            Some(p) => p,
            None => return Ok(Err(StatusCode::NOT_FOUND)),
        };

        if prop.status != "pending" {
            return Ok(Err(StatusCode::CONFLICT));
        }

        crate::db::set_proposal_status(&conn, id, "approved")?;
        Ok(Ok(()))
    })
    .await;

    match res {
        Ok(Ok(())) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "approved" }),
        ),
        Ok(Err(StatusCode::NOT_FOUND)) => json_error(
            StatusCode::NOT_FOUND,
            &format!("Không tìm thấy đề xuất #{id}"),
        ),
        Ok(Err(StatusCode::CONFLICT)) => json_error(
            StatusCode::CONFLICT,
            "Chỉ có thể duyệt đề xuất đang ở trạng thái pending",
        ),
        Ok(Err(_)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "Lỗi không xác định"),
        Err(e) => internal_error("duyệt đề xuất", &e),
    }
}

#[derive(Deserialize, Default)]
pub struct RejectBody {
    pub reason: Option<String>,
}

async fn post_proposal_reject(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    body: Option<Json<RejectBody>>,
) -> Response {
    let reason = body.and_then(|Json(b)| b.reason);
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let prop = match crate::db::get_proposal(&conn, id)? {
            Some(p) => p,
            None => return Ok(Err(StatusCode::NOT_FOUND)),
        };

        if prop.status != "pending" {
            return Ok(Err(StatusCode::CONFLICT));
        }

        crate::db::set_proposal_status_reason(&conn, id, "rejected", reason.as_deref())?;
        Ok(Ok(()))
    })
    .await;

    match res {
        Ok(Ok(())) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "rejected" }),
        ),
        Ok(Err(StatusCode::NOT_FOUND)) => json_error(
            StatusCode::NOT_FOUND,
            &format!("Không tìm thấy đề xuất #{id}"),
        ),
        Ok(Err(StatusCode::CONFLICT)) => json_error(
            StatusCode::CONFLICT,
            "Chỉ có thể từ chối đề xuất đang ở trạng thái pending",
        ),
        Ok(Err(_)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "Lỗi không xác định"),
        Err(e) => internal_error("từ chối đề xuất", &e),
    }
}

async fn get_orchestrator_status(State(st): State<Arc<AppState>>) -> Response {
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "enabled": st.cfg.orchestrator.enabled,
            "max_parallel": st.cfg.orchestrator.max_parallel,
            "auto_resume_quota": st.cfg.orchestrator.auto_resume_quota,
            "resume_after_minutes": st.cfg.orchestrator.resume_after_minutes,
        }),
    )
}
