//! API ghi chú prompt theo 4 tầng (stacks, projects, workflows, teams) và xem trước prompt (Plan B).

use crate::dashboard::settings_store::{self, WriteError};
use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/prompts", get(list_prompts))
        .route("/api/prompts/preview", get(preview_prompt))
        .route(
            "/api/prompts/{layer}/{name}/{role}",
            get(get_prompt_note)
                .put(put_prompt_note)
                .delete(delete_prompt_note),
        )
}

#[derive(serde::Deserialize)]
pub struct PutPromptBody {
    pub text: String,
    pub version: String,
}

#[derive(serde::Deserialize)]
pub struct PreviewQuery {
    #[serde(default)]
    pub project: Option<String>,
    pub role: String,
    #[serde(default)]
    pub workflow: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub member: Option<String>,
}

fn conflict(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(serde_json::json!({
            "error": message,
            "code": "conflict",
        })),
    )
        .into_response()
}

async fn list_prompts() -> Response {
    let res = blocking(move || {
        let stacks = crate::prompts::list_layer_names("stacks");
        let projects = crate::prompts::list_layer_names("projects");
        let workflows = crate::prompts::list_layer_names("workflows");
        let teams = crate::prompts::list_layer_names("teams");
        let departments = crate::prompts::list_layer_names("departments");
        let roles: Vec<&str> = crate::prompts::VALID_ROLES.to_vec();
        let builtin_stacks = crate::prompts::builtin_stacks();

        Ok(serde_json::json!({
            "layers": {
                "stacks": stacks,
                "projects": projects,
                "workflows": workflows,
                "teams": teams,
                "departments": departments,
            },
            "roles": roles,
            "builtin_stacks": builtin_stacks,
        }))
    })
    .await;

    match res {
        Ok(v) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(v),
        )
            .into_response(),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn get_prompt_note(
    AxumPath((layer, name, role)): AxumPath<(String, String, String)>,
) -> Response {
    let res = blocking(move || {
        if !crate::prompts::LAYERS.contains(&layer.as_str()) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tầng không hợp lệ"));
        }
        if !crate::prompts::valid_name(&name) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tên không hợp lệ"));
        }
        if !crate::prompts::valid_role(&role) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Vai không hợp lệ"));
        }

        let path = crate::prompts::note_path(&layer, &name, &role);
        if path.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| {
                crate::error::BrainError::error(format!("Không đọc được {}: {e}", path.display()))
            })?;
            let version = settings_store::file_version(Some(&bytes));
            let text = String::from_utf8_lossy(&bytes).into_owned();
            return Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "text": text,
                    "source": "user",
                    "version": version,
                    "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                })),
            )
                .into_response());
        }

        if layer == "stacks" {
            if let Some(builtin) = crate::prompts::builtin_stack_note(&name, &role) {
                return Ok((
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "text": builtin,
                        "source": "builtin",
                        "version": settings_store::VERSION_ABSENT,
                        "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                    })),
                )
                    .into_response());
            }
        }

        if layer == "departments" {
            if let Some(builtin) = crate::prompts::builtin_department_note(&name, &role) {
                return Ok((
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "text": builtin,
                        "source": "builtin",
                        "version": settings_store::VERSION_ABSENT,
                        "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                    })),
                )
                    .into_response());
            }
        }

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(serde_json::json!({
                "text": "",
                "source": "none",
                "version": settings_store::VERSION_ABSENT,
                "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
            })),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn put_prompt_note(
    AxumPath((layer, name, role)): AxumPath<(String, String, String)>,
    Json(body): Json<PutPromptBody>,
) -> Response {
    let res = blocking(move || {
        if !crate::prompts::LAYERS.contains(&layer.as_str()) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tầng không hợp lệ"));
        }
        if !crate::prompts::valid_name(&name) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tên không hợp lệ"));
        }
        if !crate::prompts::valid_role(&role) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Vai không hợp lệ"));
        }
        if body.text.len() > crate::prompts::PROMPT_NOTE_MAX_BYTES {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                &format!(
                    "Ghi chú quá dài: {} byte (tối đa {})",
                    body.text.len(),
                    crate::prompts::PROMPT_NOTE_MAX_BYTES
                ),
            ));
        }

        let path = crate::prompts::note_path(&layer, &name, &role);
        match settings_store::write_text(&path, &body.version, &body.text) {
            Ok(new_version) => Ok((
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "ok": true,
                    "version": new_version,
                    "text": body.text,
                    "source": "user",
                    "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                })),
            )
                .into_response()),
            Err(WriteError::Conflict) => Ok(conflict(
                "Ghi chú vừa bị thay đổi ở nơi khác — hãy tải lại và sửa lại.",
            )),
            Err(WriteError::Refused(_)) => Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Nội dung không hợp lệ",
            )),
            Err(WriteError::Io(e)) => Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, &e)),
        }
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn delete_prompt_note(
    AxumPath((layer, name, role)): AxumPath<(String, String, String)>,
) -> Response {
    let res = blocking(move || {
        if !crate::prompts::LAYERS.contains(&layer.as_str()) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tầng không hợp lệ"));
        }
        if !crate::prompts::valid_name(&name) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Tên không hợp lệ"));
        }
        if !crate::prompts::valid_role(&role) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Vai không hợp lệ"));
        }

        let path = crate::prompts::note_path(&layer, &name, &role);
        if let Err(e) = settings_store::delete_file(&path) {
            return match e {
                WriteError::Io(msg) => Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, &msg)),
                _ => Ok(json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Lỗi xóa file",
                )),
            };
        }

        if layer == "stacks" {
            if let Some(builtin) = crate::prompts::builtin_stack_note(&name, &role) {
                return Ok((
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "ok": true,
                        "text": builtin,
                        "source": "builtin",
                        "version": settings_store::VERSION_ABSENT,
                        "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                    })),
                )
                    .into_response());
            }
        }

        if layer == "departments" {
            if let Some(builtin) = crate::prompts::builtin_department_note(&name, &role) {
                return Ok((
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "ok": true,
                        "text": builtin,
                        "source": "builtin",
                        "version": settings_store::VERSION_ABSENT,
                        "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
                    })),
                )
                    .into_response());
            }
        }

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(serde_json::json!({
                "ok": true,
                "text": "",
                "source": "none",
                "version": settings_store::VERSION_ABSENT,
                "max_bytes": crate::prompts::PROMPT_NOTE_MAX_BYTES,
            })),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn preview_prompt(
    State(st): State<Arc<AppState>>,
    Query(q): Query<PreviewQuery>,
) -> Response {
    let res = blocking(move || {
        if !crate::prompts::valid_role(&q.role) {
            return Ok(json_error(StatusCode::BAD_REQUEST, "Vai không hợp lệ"));
        }

        let base = match crate::prompts::base_prompt_for(&q.role) {
            Ok(b) => b,
            Err(e) => return Ok(json_error(StatusCode::BAD_REQUEST, &e)),
        };

        let layers = if let Some(proj_id) = q.project.as_deref().filter(|s| !s.trim().is_empty()) {
            let map = match crate::projects::load_map(&st.map_path) {
                Ok(m) => m,
                Err(e) => return Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message)),
            };

            let proj = match crate::projects::select_project(proj_id, &map) {
                Ok(p) => p,
                Err(e) => return Ok(json_error(StatusCode::NOT_FOUND, &e.message)),
            };

            crate::prompts::layers_for(
                &proj,
                q.workflow.as_deref().filter(|s| !s.trim().is_empty()),
                q.team.as_deref().filter(|s| !s.trim().is_empty()),
            )
        } else {
            crate::prompts::PromptLayers {
                stacks: vec![],
                project: None,
                workflow: q.workflow.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_string),
                team: q.team.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_string),
            }
        };

        let (mut composed, mut warnings) = crate::prompts::compose(base, &q.role, &layers);
        let (extra, dept_warns) = crate::departments::prompt_append_for(&q.role, q.member.as_deref());
        warnings.extend(dept_warns);
        if !extra.trim().is_empty() {
            if !composed.ends_with('\n') {
                composed.push('\n');
            }
            composed.push('\n');
            composed.push_str(extra.trim());
            composed.push('\n');
        }

        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            Json(serde_json::json!({
                "prompt": composed,
                "warnings": warnings,
            })),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}
