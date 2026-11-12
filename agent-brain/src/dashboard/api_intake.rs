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

pub const INPUTS_BODY_LIMIT: usize = 512 * 1024;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/intake",
            get(list_intake)
                .post(create_intake)
                .layer(axum::extract::DefaultBodyLimit::max(INPUTS_BODY_LIMIT)),
        )
        .route("/api/intake/{id}/cancel", post(cancel_intake))
        .route("/api/intake/{id}/retry", post(retry_intake))
        .route("/api/proposals", get(list_proposals))
        .route("/api/proposals/{id}/priority", post(post_proposal_priority))
        .route("/api/proposals/{id}/approve", post(post_proposal_approve))
        .route("/api/proposals/{id}/reject", post(post_proposal_reject))
        .route("/api/orchestrator/status", get(get_orchestrator_status))
        .route("/api/orchestrator/board", get(get_orchestrator_board))
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

pub use crate::inputs::InputItem;

#[derive(Deserialize)]
pub struct CreateIntakeBody {
    pub request: String,
    pub project: Option<serde_json::Value>,
    pub priority: Option<String>,
    pub split: Option<bool>,
    #[serde(default)]
    pub inputs: Vec<InputItem>,
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

    let secrets = crate::inputs::collect_secrets(&st);
    if let Err(err_msg) = crate::inputs::validate_inputs(&body.inputs, &secrets) {
        return json_error(StatusCode::UNPROCESSABLE_ENTITY, &err_msg);
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
    let input_docs = body.inputs;

    let res = blocking(move || {
        let mut conn = crate::db::open(&st.db_path)?;
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

        let map = crate::projects::load_map(&st.map_path)?;
        // A hidden project cannot be chosen for new work.
        if let Some(pid) = project_id {
            let slug: Option<String> = conn
                .query_row(
                    "SELECT slug FROM projects WHERE id = ?1",
                    rusqlite::params![pid],
                    |r| r.get(0),
                )
                .ok();
            if let Some(sl) = slug {
                if crate::projects::projects_of(&map)
                    .iter()
                    .any(|p| p.id == sl && p.hidden)
                {
                    return Err(crate::error::BrainError::new(
                        format!("Dự án '{sl}' đã ẩn — bỏ ẩn ở màn Dự án rồi giao việc"),
                        crate::error::EXIT_ERROR,
                    ));
                }
            }
        }
        let split = match body.split {
            Some(s) => s,
            None => match project_id {
                Some(pid) => {
                    let slug: Option<String> = conn
                        .query_row(
                            "SELECT slug FROM projects WHERE id = ?1",
                            rusqlite::params![pid],
                            |r| r.get(0),
                        )
                        .ok();
                    if let Some(ref sl) = slug {
                        crate::projects::triage_split_of(&map, sl)
                    } else {
                        true
                    }
                }
                None => true,
            },
        };

        let tx = conn.transaction()?;
        let id = crate::db::insert_intake(&tx, "ui", project_id, &req_text, &priority, split)?;
        for doc in &input_docs {
            crate::inputs::add_to_intake(&tx, id, &doc.name, &doc.content, &secrets)
                .map_err(crate::error::BrainError::error)?;
        }
        tx.commit()?;
        Ok(id)
    })
    .await;

    match res {
        Ok(id) => json_response(StatusCode::CREATED, serde_json::json!({ "id": id })),
        Err(e) => {
            if e.message.contains("Không tìm thấy dự án")
                || e.message.contains("đã ẩn")
                || e.message.contains("tài liệu")
                || e.message.contains("bí mật")
                || e.message.contains("64 KiB")
                || e.message.contains("Tên tài liệu")
            {
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
                return Ok(Err((
                    StatusCode::NOT_FOUND,
                    format!("Không tìm thấy intake #{id}"),
                )));
            }
        };

        if intake.status != "queued" && intake.status != "failed" {
            return Ok(Err((
                StatusCode::CONFLICT,
                "Chỉ huỷ được việc đang chờ hoặc bị lỗi".to_string(),
            )));
        }

        let cancelled = crate::db::cancel_intake_if_cancellable(&conn, id)?;
        if !cancelled {
            return Ok(Err((
                StatusCode::CONFLICT,
                "Chỉ huỷ được việc đang chờ hoặc bị lỗi".to_string(),
            )));
        }

        Ok(Ok(intake))
    })
    .await;

    match res {
        Ok(Ok(_)) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "cancelled" }),
        ),
        Ok(Err((code, msg))) => json_error(code, &msg),
        Err(e) => internal_error("hủy intake", &e),
    }
}

const RETRY_ONLY_FAILED: &str = "Chỉ thử lại được việc bị lỗi";

/// `failed` → `queued`, error cleared: the queue triages it again on its next tick.
async fn retry_intake(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        if crate::db::get_intake(&conn, id)?.is_none() {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                format!("Không tìm thấy intake #{id}"),
            )));
        }
        if !crate::db::retry_intake_if_failed(&conn, id)? {
            return Ok(Err((StatusCode::CONFLICT, RETRY_ONLY_FAILED.to_string())));
        }
        Ok(Ok(()))
    })
    .await;

    match res {
        Ok(Ok(())) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "queued" }),
        ),
        Ok(Err((code, msg))) => json_error(code, &msg),
        Err(e) => internal_error("thử lại intake", &e),
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
        let proposals = crate::db::list_proposals(&conn, query.status.as_deref())?;
        let mut list = Vec::with_capacity(proposals.len());
        for p in proposals {
            let input_names: Vec<String> = crate::inputs::list_for_proposal(&conn, p.id)
                .unwrap_or_default()
                .into_iter()
                .map(|inp| inp.name)
                .collect();
            let mut val = serde_json::to_value(&p)
                .map_err(|e| crate::error::BrainError::error(e.to_string()))?;
            val["inputs"] = serde_json::json!(input_names);
            list.push(val);
        }
        Ok(list)
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

#[derive(Deserialize, Default)]
pub struct ApproveProposalBody {
    pub team: Option<String>,
    pub workflow: Option<String>,
    pub priority: Option<String>,
    pub title: Option<String>,
}

/// Field overrides when approving a proposal. `Default` approves it as proposed.
#[derive(Default)]
pub struct ProposalOverrides {
    pub team: Option<String>,
    pub workflow: Option<String>,
    pub priority: Option<String>,
    pub title: Option<String>,
}

/// Approves a `pending` proposal through the same path the dashboard uses (including
/// `triage::validate`). The outer `Err` is an internal error; the inner `Err` carries the
/// HTTP status and the user-facing message the dashboard returns.
pub(crate) fn approve_pending_proposal(
    st: &AppState,
    id: i64,
    over: ProposalOverrides,
) -> crate::error::BrainResult<Result<(), (StatusCode, String)>> {
    let conn = crate::db::open(&st.db_path)?;
    let prop = match crate::db::get_proposal(&conn, id)? {
        Some(p) => p,
        None => {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                format!("Không tìm thấy đề xuất #{id}"),
            )));
        }
    };

    if prop.status != "pending" {
        return Ok(Err((
            StatusCode::CONFLICT,
            "Chỉ có thể duyệt đề xuất đang ở trạng thái pending".to_string(),
        )));
    }

    let team = over.team.unwrap_or(prop.team);
    let workflow = over.workflow.unwrap_or(prop.workflow);
    let priority = over.priority.unwrap_or(prop.priority);
    let title = over.title.unwrap_or(prop.title);

    let project_slug: String = match conn.query_row(
        "SELECT slug FROM projects WHERE id = ?1",
        rusqlite::params![prop.project_id],
        |r| r.get(0),
    ) {
        Ok(s) => s,
        Err(_) => {
            return Ok(Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Không tìm thấy dự án với ID {}", prop.project_id),
            )));
        }
    };

    let draft = crate::orchestrator::triage::ProposalDraft {
        project: project_slug,
        title: title.clone(),
        request: prop.request.clone(),
        priority: priority.clone(),
        level: prop.level.clone(),
        team: team.clone(),
        workflow: workflow.clone(),
        reason: prop.reason.clone(),
        after: None,
        orig_index: None,
    };

    let map = crate::projects::load_map(&st.map_path)?;
    let projects = crate::projects::projects_of(&map);
    let (teams_loaded, _) = crate::teams::load();
    let workflows_loaded = crate::workflows::load(&st.cfg);

    if let Err(err_msg) = crate::orchestrator::triage::validate(
        &draft,
        &projects,
        &teams_loaded,
        &workflows_loaded,
        &st.cfg.engines,
    ) {
        return Ok(Err((StatusCode::UNPROCESSABLE_ENTITY, err_msg)));
    }

    let updated = crate::db::approve_proposal_with(
        &conn,
        id,
        &team,
        &workflow,
        &priority,
        &title,
    )?;
    if !updated {
        return Ok(Err((
            StatusCode::CONFLICT,
            "Chỉ có thể duyệt đề xuất đang ở trạng thái pending".to_string(),
        )));
    }

    Ok(Ok(()))
}

async fn post_proposal_approve(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    bytes: axum::body::Bytes,
) -> Response {
    let b: ApproveProposalBody = if bytes.is_empty() {
        ApproveProposalBody::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(b) => b,
            Err(e) => {
                return json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &format!("Dữ liệu gửi lên không đúng định dạng JSON: {e}"),
                );
            }
        }
    };

    let over = ProposalOverrides {
        team: b.team,
        workflow: b.workflow,
        priority: b.priority,
        title: b.title,
    };
    let res = blocking(move || approve_pending_proposal(&st, id, over)).await;

    match res {
        Ok(Ok(())) => json_response(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "approved" }),
        ),
        Ok(Err((code, msg))) => json_error(code, &msg),
        Err(e) => internal_error("duyệt đề xuất", &e),
    }
}

#[derive(Deserialize, Default)]
pub struct RejectBody {
    pub reason: Option<String>,
}

/// Rejects a `pending` proposal. The inner `Err` is the HTTP status the dashboard returns
/// (`NOT_FOUND` when the proposal does not exist, `CONFLICT` when it is not `pending`).
pub(crate) fn reject_pending_proposal(
    st: &AppState,
    id: i64,
    reason: Option<&str>,
) -> crate::error::BrainResult<Result<(), StatusCode>> {
    let conn = crate::db::open(&st.db_path)?;
    let prop = match crate::db::get_proposal(&conn, id)? {
        Some(p) => p,
        None => return Ok(Err(StatusCode::NOT_FOUND)),
    };

    if prop.status != "pending" {
        return Ok(Err(StatusCode::CONFLICT));
    }

    crate::db::set_proposal_status_reason(&conn, id, "rejected", reason)?;
    Ok(Ok(()))
}

async fn post_proposal_reject(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    body: Option<Json<RejectBody>>,
) -> Response {
    let reason = body.and_then(|Json(b)| b.reason);
    let res = blocking(move || reject_pending_proposal(&st, id, reason.as_deref())).await;

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
    let orch = st.orch();
    json_response(
        StatusCode::OK,
        serde_json::json!({
            "enabled": orch.enabled,
            "max_parallel": orch.max_parallel,
            "auto_resume_quota": orch.auto_resume_quota,
            "resume_after_minutes": orch.resume_after_minutes,
        }),
    )
}

async fn get_orchestrator_board(State(st): State<Arc<AppState>>) -> Response {
    let st_clone = st.clone();
    let res = blocking(move || {
        let conn = crate::db::open(&st_clone.db_path)?;
        let orch = st_clone.orch();
        let active =
            crate::orchestrator::scheduler::active_task_count(&st_clone, &conn).unwrap_or(0);
        let status = serde_json::json!({
            "enabled": orch.enabled,
            "max_parallel": orch.max_parallel,
            "active": active,
            "auto_resume_quota": orch.auto_resume_quota,
            "resume_after_minutes": orch.resume_after_minutes,
        });

        let proj_map = crate::db::list_project_display_map(&conn)?;

        let raw_intake = crate::db::list_orchestrator_board_intake(&conn)?;
        let intake_list: Vec<serde_json::Value> = raw_intake
            .into_iter()
            .map(|it| {
                let (slug, name) = it
                    .project_id
                    .and_then(|pid| proj_map.get(&pid).cloned())
                    .map(|(s, n)| (Some(s), Some(n)))
                    .unwrap_or((None, None));
                serde_json::json!({
                    "id": it.id,
                    "source": it.source,
                    "project_id": it.project_id,
                    "project": slug,
                    "project_name": name,
                    "request": it.request,
                    "priority": it.priority,
                    "status": it.status,
                    "error": it.error,
                    "created_at": it.created_at,
                    "updated_at": it.updated_at,
                })
            })
            .collect();

        let raw_proposals = crate::db::list_orchestrator_board_proposals(&conn)?;
        let proposal_list: Vec<serde_json::Value> = raw_proposals
            .into_iter()
            .map(|p| {
                let (slug, name) = proj_map
                    .get(&p.project_id)
                    .cloned()
                    .unwrap_or_else(|| (p.project_id.to_string(), p.project_id.to_string()));
                let input_names: Vec<String> = crate::inputs::list_for_proposal(&conn, p.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|inp| inp.name)
                    .collect();
                serde_json::json!({
                    "id": p.id,
                    "intake_id": p.intake_id,
                    "project_id": p.project_id,
                    "project": slug,
                    "project_name": name,
                    "title": p.title,
                    "request": p.request,
                    "priority": p.priority,
                    "team": p.team,
                    "workflow": p.workflow,
                    "reason": p.reason,
                    "status": p.status,
                    "task_id": p.task_id,
                    "error": p.error,
                    "created_at": p.created_at,
                    "updated_at": p.updated_at,
                    "after_proposal_id": p.after_proposal_id,
                    "inputs": input_names,
                })
            })
            .collect();

        let raw_quota_stops = crate::db::list_orchestrator_board_quota_stops(&conn)?;
        let auto_resume = orch.auto_resume_quota;
        let resume_mins = orch.resume_after_minutes as i64;
        let quota_list: Vec<serde_json::Value> = raw_quota_stops
            .into_iter()
            .map(|qs| {
                let resume_at = if auto_resume {
                    if qs.stopped_at < 10_000_000_000 {
                        Some(qs.stopped_at + resume_mins * 60)
                    } else {
                        // C3: the same due time the scheduler uses (reset time when later).
                        Some(crate::quota::resume_due_ms(
                            qs.stopped_at,
                            resume_mins * 60 * 1000,
                            crate::db::latest_quota_reset_at(&conn, qs.task_id),
                        ))
                    }
                } else {
                    None
                };
                serde_json::json!({
                    "task_id": qs.task_id,
                    "stopped_at": qs.stopped_at,
                    "engine": qs.engine,
                    "resume_at": resume_at,
                })
            })
            .collect();

        let recent_items = crate::db::list_orchestrator_board_recent(&conn, &proj_map)?;

        let (teams_loaded, _) = crate::teams::load();
        let team_names: Vec<String> = teams_loaded.teams.into_iter().map(|t| t.name).collect();
        let workflows_loaded = crate::workflows::load(&st_clone.cfg);
        let workflow_names: Vec<String> = workflows_loaded
            .workflows
            .into_iter()
            .map(|w| w.name)
            .collect();

        Ok::<_, crate::error::BrainError>(serde_json::json!({
            "status": status,
            "intake": intake_list,
            "proposals": proposal_list,
            "quota_stops": quota_list,
            "recent": recent_items,
            "teams": team_names,
            "workflows": workflow_names,
        }))
    })
    .await;

    match res {
        Ok(data) => json_response(StatusCode::OK, data),
        Err(e) => internal_error("lấy dữ liệu bảng điều phối", &e),
    }
}
