//! Owning plan: 05-07

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Json;
use axum::Router;
use std::hash::{DefaultHasher, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

pub const SUGGEST_LIMIT: usize = 10;

static WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn write_mutex() -> &'static Mutex<()> {
    WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

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

fn project_json(p: &crate::projects::Project, map: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "name_vi": p.name_vi,
        "aliases": p.aliases,
        "path": p.path,
        "type": p.kind,
        "profile": p.profile,
        "repo": p.repo,
        "missing": p.missing,
        "workflow": crate::projects::workflow_of(map, &p.id),
    })
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAliasesBody {
    pub aliases: Vec<String>,
    pub etag: String,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScanBody {}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/projects", get(list_projects))
        .route("/api/projects/suggest", get(suggest_projects))
        .route("/api/projects/scan", post(scan_projects))
        .route("/api/projects/{id}", put(update_aliases))
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
            .map(|p| project_json(&p, &map))
            .collect();

        let body = serde_json::json!({
            "etag": etag,
            "scan_roots": scan_roots,
            "projects": projects_json,
            "duplicates": duplicates,
        });

        Ok(Json(body).into_response())
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
                    "workflow": crate::projects::workflow_of(&map, &s.project.id),
                })
            })
            .collect();

        Ok(Json(serde_json::json!({ "items": items })).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn update_aliases(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateAliasesBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if body.etag != current_etag {
            let map = crate::projects::load_map(&st.map_path)?;
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
                .map(|(key, ids)| serde_json::json!({ "key": key, "ids": ids }))
                .collect();
            let projects = crate::projects::projects_of(&map);
            let projects_json: Vec<serde_json::Value> = projects
                .into_iter()
                .map(|p| project_json(&p, &map))
                .collect();

            let conflict_body = serde_json::json!({
                "error": "projects_map.json vừa thay đổi ở nơi khác — đã tải lại danh sách, hãy sửa lại",
                "etag": current_etag,
                "scan_roots": scan_roots,
                "projects": projects_json,
                "duplicates": duplicates,
            });

            return Ok((
                StatusCode::CONFLICT,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(conflict_body),
            )
                .into_response());
        }

        let mut data = crate::projects::load_map(&st.map_path)?;

        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;

        let project_entry = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id));

        let project_obj = match project_entry {
            Some(obj) => obj,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        // Clean aliases: trim, drop empty, length <= 100, de-duplicate by normalize
        let mut cleaned_aliases = Vec::new();
        let mut seen_norm = std::collections::HashSet::new();

        for a in body.aliases {
            let trimmed = a.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.chars().count() > 100 {
                return Ok(json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Alias quá dài (tối đa 100 ký tự)",
                ));
            }
            let norm = crate::routing::normalize(trimmed);
            if seen_norm.insert(norm) {
                cleaned_aliases.push(trimmed.to_string());
            }
        }

        project_obj["aliases"] = serde_json::Value::Array(
            cleaned_aliases
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        );

        crate::projects::save_map(&st.map_path, &data)?;

        if let Ok(conn) = crate::db::open(&st.db_path) {
            crate::db::sync_projects(&conn, &crate::projects::projects_of(&data))?;
        }

        let new_etag = map_etag(&st.map_path);

        let raw_projects = data
            .get("projects")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        let duplicates_raw = crate::projects::find_duplicates(&raw_projects);
        let mut warnings = Vec::new();
        for (key, ids) in &duplicates_raw {
            let ids_str = ids.join(", ");
            warnings.push(format!("CẢNH BÁO: tên/alias trùng \"{key}\": {ids_str}"));
        }
        let duplicates: Vec<serde_json::Value> = duplicates_raw
            .into_iter()
            .map(|(key, ids)| serde_json::json!({ "key": key, "ids": ids }))
            .collect();

        let scan_roots = data
            .get("scan_roots")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        let projects = crate::projects::projects_of(&data);
        let projects_json: Vec<serde_json::Value> = projects
            .into_iter()
            .map(|p| project_json(&p, &data))
            .collect();

        let body = serde_json::json!({
            "etag": new_etag,
            "scan_roots": scan_roots,
            "projects": projects_json,
            "duplicates": duplicates,
            "warnings": warnings,
        });

        Ok(Json(body).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn scan_projects(State(st): State<Arc<AppState>>, _body: Option<Json<ScanBody>>) -> Response {
    let res = blocking(move || {
        let mut cmd = std::process::Command::new(&st.exe);
        cmd.args(["projects", "scan"]);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd
            .output()
            .map_err(|e| crate::error::BrainError::error(format!("Không chạy được scan: {e}")))?;

        let exit_code = output.status.code().unwrap_or(1);
        let stdout_lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();
        let stderr_lines: Vec<String> = String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(|s| s.to_string())
            .collect();

        if let Ok(map) = crate::projects::load_map(&st.map_path) {
            if let Ok(conn) = crate::db::open(&st.db_path) {
                let _ = crate::db::sync_projects(&conn, &crate::projects::projects_of(&map));
            }
        }

        let body = serde_json::json!({
            "exit_code": exit_code,
            "stdout": stdout_lines,
            "stderr": stderr_lines,
        });

        Ok(Json(body).into_response())
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

        Ok(Json(body).into_response())
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_json_includes_workflow() {
        let map = serde_json::json!({
            "projects": [
                {
                    "id": "p1",
                    "path": "/tmp/p1",
                    "name_vi": "P1",
                    "workflow": "nhanh"
                }
            ]
        });
        let projs = crate::projects::projects_of(&map);
        let j = project_json(&projs[0], &map);
        assert_eq!(j["workflow"], "nhanh");
    }
}
