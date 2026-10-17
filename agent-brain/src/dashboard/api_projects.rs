//! Owning plan: 05-07

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::hash::{DefaultHasher, Hasher};
use std::path::Path;
use std::sync::Arc;

pub const SUGGEST_LIMIT: usize = 10;

pub fn map_etag(path: &Path) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return "absent".to_string(),
    };
    let mut hasher = DefaultHasher::new();
    hasher.write(&bytes);
    let hash = hasher.finish();
    format!("{:016x}-{}", hash, bytes.len())
}

#[derive(serde::Deserialize)]
struct ProjectsQuery {
    profile: Option<String>,
}

#[derive(serde::Deserialize)]
struct SuggestQuery {
    #[serde(default)]
    q: String,
    profile: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/projects", get(list_projects))
        .route("/api/projects/suggest", get(suggest_projects))
        .route("/api/projects/{id}/status", get(project_status))
}

async fn list_projects(
    State(st): State<Arc<AppState>>,
    Query(query): Query<ProjectsQuery>,
) -> Response {
    let res = blocking(move || {
        let profile_opt = query.profile.as_deref().filter(|s| !s.is_empty());
        if let Some(p) = profile_opt {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return Ok(json_error(StatusCode::BAD_REQUEST, &e.message));
            }
        }

        let map = crate::projects::load_map(&st.map_path)?;
        let etag = map_etag(&st.map_path);

        let scan_roots = map
            .get("scan_roots")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        let raw_projects = map
            .get("projects")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();

        let duplicates_raw = crate::projects::find_duplicates(&raw_projects);
        let duplicates: Vec<serde_json::Value> = duplicates_raw
            .into_iter()
            .map(|(key, ids)| {
                serde_json::json!({
                    "key": key,
                    "ids": ids,
                })
            })
            .collect();

        let mut projects = crate::projects::projects_of(&map);
        if let Some(prof) = profile_opt {
            projects.retain(|p| p.profile == prof);
        }

        let projects_json: Vec<serde_json::Value> = projects
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name_vi": p.name_vi,
                    "aliases": p.aliases,
                    "path": p.path,
                    "type": p.kind,
                    "profile": p.profile,
                    "repo": p.repo,
                    "missing": p.missing,
                })
            })
            .collect();

        let body = serde_json::json!({
            "etag": etag,
            "scan_roots": scan_roots,
            "projects": projects_json,
            "duplicates": duplicates,
        });

        Ok(axum::Json(body).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn suggest_projects(
    State(st): State<Arc<AppState>>,
    Query(query): Query<SuggestQuery>,
) -> Response {
    let res = blocking(move || {
        let profile_opt = query.profile.as_deref().filter(|s| !s.is_empty());
        if let Some(p) = profile_opt {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return Ok(json_error(StatusCode::BAD_REQUEST, &e.message));
            }
        }

        let map = crate::projects::load_map(&st.map_path)?;
        let mut projects = crate::projects::projects_of(&map);
        if let Some(prof) = profile_opt {
            projects.retain(|p| p.profile == prof);
        }

        let suggestions = crate::routing::suggest(&query.q, &projects);
        let items: Vec<serde_json::Value> = suggestions
            .into_iter()
            .take(SUGGEST_LIMIT)
            .map(|s| {
                serde_json::json!({
                    "id": s.project.id,
                    "name_vi": s.project.name_vi,
                    "aliases": s.project.aliases,
                    "profile": s.project.profile,
                    "missing": s.project.missing,
                    "rank": s.rank,
                    "matched": s.matched,
                })
            })
            .collect();

        Ok(axum::Json(serde_json::json!({ "items": items })).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn project_status(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let res = blocking(move || {
        let map = crate::projects::load_map(&st.map_path)?;
        let projects = crate::projects::projects_of(&map);

        let project = match projects.into_iter().find(|p| p.id == id) {
            Some(p) => p,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        let root = project.root();
        if !root.is_dir() {
            return Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!(
                    "Đường dẫn dự án '{}' không tồn tại: {}",
                    project.id,
                    root.display()
                ),
            ));
        }

        if !crate::gitops::is_repo_root(&root) {
            return Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!(
                    "Thư mục dự án '{}' không phải repo git: {}",
                    project.id,
                    root.display()
                ),
            ));
        }

        let branch =
            crate::gitops::current_branch(&root).unwrap_or_else(|| "(detached)".to_string());

        let status_out = crate::gitops::git(&root, &["status", "--short"]).unwrap_or_default();
        let clean = status_out.trim().is_empty();
        let status_lines: Vec<String> = status_out
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let log_lines: Vec<String> = match crate::gitops::git(&root, &["log", "--oneline", "-5"]) {
            Ok(out) => out
                .lines()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            Err(_) => vec![],
        };

        let conn = crate::db::open(&st.db_path).ok();
        let task_opt = conn.as_ref().and_then(|c| {
            let pid = crate::db::project_row_id(c, &project.id).ok()?;
            crate::db::find_task_by_branch(c, pid, &branch)
                .ok()
                .flatten()
        });

        let (base_commit, task_json) = match task_opt {
            Some(task) => {
                let base = task
                    .base_commit
                    .clone()
                    .unwrap_or_else(|| "HEAD".to_string());
                let t_json = serde_json::json!({
                    "id": task.id,
                    "branch": task.branch,
                    "status": task.status,
                });
                (base, Some(t_json))
            }
            None => ("HEAD".to_string(), None),
        };

        let diff_lines: Vec<String> =
            match crate::gitops::git(&root, &["diff", "--stat", &base_commit]) {
                Ok(out) => out
                    .lines()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                Err(_) => vec![],
            };

        let body = serde_json::json!({
            "project": project.id,
            "path": root.to_string_lossy().to_string(),
            "branch": branch,
            "clean": clean,
            "status": status_lines,
            "log": log_lines,
            "diff_stat": diff_lines,
            "task": task_json,
        });

        Ok(axum::Json(body).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}
