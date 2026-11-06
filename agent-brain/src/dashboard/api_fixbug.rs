//! API quản lý và lấy bug theo dự án (/api/fixbug/{project}) cho dashboard (Plan fixbug-sources-engine).

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/fixbug/{project}", get(get_fixbug_status))
        .route("/api/fixbug/{project}/pull", post(post_fixbug_pull))
        .route("/api/fixbug/{project}/deployed", post(post_fixbug_deployed))
        .route("/api/fixbug/{project}/reject", post(post_fixbug_reject))
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

#[allow(clippy::result_large_err)]
fn resolve_project_context(
    st: &AppState,
    project_query: &str,
) -> Result<(crate::projects::Project, i64, PathBuf), Response> {
    let map = crate::projects::load_map(&st.map_path).map_err(|e| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không đọc được danh sách dự án: {e}"),
        )
    })?;
    let projects = crate::projects::projects_of(&map);
    let project = crate::routing::resolve(project_query, &projects).map_err(|_| {
        json_error(
            StatusCode::NOT_FOUND,
            &format!("Không tìm thấy dự án '{project_query}'"),
        )
    })?;

    let conn = crate::db::open(&st.db_path).map_err(|e| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không mở được CSDL: {e}"),
        )
    })?;
    let project_id = crate::db::project_row_id(&conn, &project.id).map_err(|e| {
        json_error(
            StatusCode::NOT_FOUND,
            &format!("Dự án '{}' chưa có trong CSDL: {e}", project.id),
        )
    })?;

    let root = PathBuf::from(&project.path);
    Ok((project, project_id, root))
}

#[derive(Serialize)]
struct SourceSummary {
    r#type: String,
    summary: String,
}

#[derive(Serialize)]
struct FixbugStatusResponse {
    configured: bool,
    sources: Vec<SourceSummary>,
    refs: Vec<crate::db::FixbugRefRow>,
}

async fn get_fixbug_status(
    State(st): State<Arc<AppState>>,
    AxumPath(project_query): AxumPath<String>,
) -> Response {
    let (_proj, project_id, root) = match resolve_project_context(&st, &project_query) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let db_path = st.db_path.clone();

    let result = blocking(move || {
        let conn = crate::db::open(&db_path)?;
        let refs = crate::db::list_fixbug_refs(&conn, project_id)?;

        let (configured, sources) = match crate::fixbug::config::load_config(&root) {
            Ok(cfg) => {
                let mut list = Vec::new();
                for src in cfg.sources {
                    match src {
                        crate::fixbug::config::SourceConfig::Trello(t) => {
                            list.push(SourceSummary {
                                r#type: "trello".to_string(),
                                summary: format!(
                                    "Trello board: {}, intake lists: {:?}",
                                    t.board.unwrap_or_else(|| "(từ .env)".to_string()),
                                    t.intake_lists
                                ),
                            });
                        }
                        crate::fixbug::config::SourceConfig::Gsheet(g) => {
                            list.push(SourceSummary {
                                r#type: "gsheet".to_string(),
                                summary: format!("Google Sheet: {}", g.url),
                            });
                        }
                    }
                }
                (true, list)
            }
            Err(_) => (false, Vec::new()),
        };

        Ok(FixbugStatusResponse {
            configured,
            sources,
            refs,
        })
    })
    .await;

    match result {
        Ok(data) => json_response(StatusCode::OK, data),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn post_fixbug_pull(
    State(st): State<Arc<AppState>>,
    AxumPath(project_query): AxumPath<String>,
) -> Response {
    let (proj, project_id, root) = match resolve_project_context(&st, &project_query) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let db_path = st.db_path.clone();

    let result = blocking(move || {
        let mut conn = crate::db::open(&db_path)?;
        let report = crate::fixbug::pull::pull(
            &mut conn,
            &root,
            project_id,
            &proj.name_vi,
            None,
        )
        .map_err(|e| crate::error::BrainError::error(e.to_string()))?;
        Ok(report)
    })
    .await;

    match result {
        Ok(report) => json_response(StatusCode::OK, report),
        Err(e) => json_error(StatusCode::BAD_REQUEST, &e.message),
    }
}

#[derive(Deserialize)]
struct DeployedBody {
    keys: Vec<String>,
    note: Option<String>,
}

async fn post_fixbug_deployed(
    State(st): State<Arc<AppState>>,
    AxumPath(project_query): AxumPath<String>,
    Json(body): Json<DeployedBody>,
) -> Response {
    let (_proj, project_id, root) = match resolve_project_context(&st, &project_query) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let db_path = st.db_path.clone();

    let result = blocking(move || {
        let mut conn = crate::db::open(&db_path)?;
        let items = crate::fixbug::writeback::deployed(
            &mut conn,
            &root,
            project_id,
            &body.keys,
            body.note.as_deref(),
        )
        .map_err(|e| crate::error::BrainError::error(e.to_string()))?;
        Ok(items)
    })
    .await;

    match result {
        Ok(items) => json_response(StatusCode::OK, items),
        Err(e) => json_error(StatusCode::BAD_REQUEST, &e.message),
    }
}

#[derive(Deserialize)]
struct RejectBody {
    keys: Vec<String>,
    reason: String,
}

async fn post_fixbug_reject(
    State(st): State<Arc<AppState>>,
    AxumPath(project_query): AxumPath<String>,
    Json(body): Json<RejectBody>,
) -> Response {
    let (_proj, project_id, root) = match resolve_project_context(&st, &project_query) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let db_path = st.db_path.clone();

    let result = blocking(move || {
        let mut conn = crate::db::open(&db_path)?;
        let items = crate::fixbug::writeback::reject(
            &mut conn,
            &root,
            project_id,
            &body.keys,
            &body.reason,
        )
        .map_err(|e| crate::error::BrainError::error(e.to_string()))?;
        Ok(items)
    })
    .await;

    match result {
        Ok(items) => json_response(StatusCode::OK, items),
        Err(e) => json_error(StatusCode::BAD_REQUEST, &e.message),
    }
}
