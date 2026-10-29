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

fn telegram_ids(map: &serde_json::Value) -> Vec<String> {
    map.get("telegram_projects")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn telegram_count(map: &serde_json::Value) -> usize {
    let ids = telegram_ids(map);
    crate::projects::projects_of(map)
        .iter()
        .filter(|p| ids.iter().any(|i| i == &p.id))
        .count()
}

fn project_json(p: &crate::projects::Project, map: &serde_json::Value) -> serde_json::Value {
    let on_telegram = telegram_ids(map).iter().any(|i| i == &p.id);
    serde_json::json!({
        "telegram": on_telegram,
        "telegram_offerable": p.kind != "media" && !p.missing,
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

fn projects_body(map: &serde_json::Value, etag: String) -> serde_json::Value {
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
    let duplicates: Vec<serde_json::Value> = crate::projects::find_duplicates(&raw_projects)
        .into_iter()
        .map(|(key, ids)| serde_json::json!({ "key": key, "ids": ids }))
        .collect();
    let projects_json: Vec<serde_json::Value> = crate::projects::projects_of(map)
        .into_iter()
        .map(|p| project_json(&p, map))
        .collect();
    serde_json::json!({
        "etag": etag,
        "scan_roots": scan_roots,
        "projects": projects_json,
        "duplicates": duplicates,
        "telegram_count": telegram_count(map),
        "telegram_max": crate::telegram::intake::MAX_OFFERS,
    })
}

fn conflict_response(st: &AppState, current_etag: String) -> crate::error::BrainResult<Response> {
    let map = crate::projects::load_map(&st.map_path)?;
    let mut body = projects_body(&map, current_etag);
    body["error"] = serde_json::json!(
        "projects_map.json vừa thay đổi ở nơi khác — đã tải lại danh sách, hãy sửa lại"
    );
    Ok((
        StatusCode::CONFLICT,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(body),
    )
        .into_response())
}

/// Saves the map, syncs SQLite and builds the shared success body.
fn save_and_respond(
    st: &AppState,
    data: &serde_json::Value,
    status: StatusCode,
    extra: Option<(&str, serde_json::Value)>,
) -> crate::error::BrainResult<Response> {
    crate::projects::save_map(&st.map_path, data)?;
    if let Ok(conn) = crate::db::open(&st.db_path) {
        crate::db::sync_projects(&conn, &crate::projects::projects_of(data))?;
    }
    let new_etag = map_etag(&st.map_path);
    let mut body = projects_body(data, new_etag);
    let raw_projects = data
        .get("projects")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let warnings: Vec<String> = crate::projects::find_duplicates(&raw_projects)
        .iter()
        .map(|(key, ids)| format!("CẢNH BÁO: tên/alias trùng \"{key}\": {}", ids.join(", ")))
        .collect();
    body["warnings"] = serde_json::json!(warnings);
    if let Some((k, v)) = extra {
        body[k] = v;
    }
    Ok((status, Json(body)).into_response())
}

const MAX_SCAN_ROOTS: usize = 20;
const MAX_PATH_CHARS: usize = 500;
const MAX_DIR_ENTRIES: usize = 300;

/// Absolute or `~/` path, no control chars, no `..`. Returns the expanded path.
fn parse_path(raw: &str) -> Result<std::path::PathBuf, String> {
    if raw.chars().count() > MAX_PATH_CHARS {
        return Err(format!("Đường dẫn quá dài (tối đa {MAX_PATH_CHARS} ký tự)"));
    }
    if raw.chars().any(|c| c.is_control()) {
        return Err("Đường dẫn chứa ký tự điều khiển".to_string());
    }
    if !(raw.starts_with('/') || raw.starts_with("~/") || raw == "~") {
        return Err(format!(
            "Đường dẫn phải tuyệt đối hoặc bắt đầu bằng ~/: {raw}"
        ));
    }
    let expanded = crate::config::expand_tilde(raw, &crate::config::home_dir());
    if expanded
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!("Đường dẫn không được chứa '..': {raw}"));
    }
    Ok(expanded)
}

fn parse_dir(raw: &str) -> Result<std::path::PathBuf, String> {
    let p = parse_path(raw)?;
    if !p.is_dir() {
        return Err(format!("Thư mục không tồn tại: {raw}"));
    }
    Ok(p)
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

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateTelegramBody {
    pub enabled: bool,
    pub etag: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateScanRootsBody {
    pub roots: Vec<String>,
    pub etag: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddProjectBody {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub etag: String,
}

#[derive(serde::Deserialize)]
struct FsDirsQuery {
    path: Option<String>,
    hidden: Option<String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScanBody {}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/projects", get(list_projects).post(add_project_api))
        .route("/api/fs/dirs", get(fs_dirs))
        .route("/api/projects/scan-roots", put(update_scan_roots))
        .route("/api/projects/suggest", get(suggest_projects))
        .route("/api/projects/scan", post(scan_projects))
        .route("/api/projects/{id}", put(update_aliases))
        .route("/api/projects/{id}/telegram", put(update_telegram))
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

        let mut projects = crate::projects::projects_of(&map);
        let mut body = projects_body(&map, etag);
        if let Some(prof) = profile_opt {
            projects.retain(|p| p.profile == prof);
            let filtered: Vec<serde_json::Value> =
                projects.iter().map(|p| project_json(p, &map)).collect();
            body["projects"] = serde_json::json!(filtered);
        }

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
            return conflict_response(&st, current_etag);
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

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn update_telegram(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateTelegramBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if body.etag != current_etag {
            return conflict_response(&st, current_etag);
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let project = match crate::projects::projects_of(&data)
            .into_iter()
            .find(|p| p.id == id)
        {
            Some(p) => p,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        let already = telegram_ids(&data).iter().any(|i| i == &id);
        if body.enabled {
            if project.kind == "media" {
                return Ok(json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Dự án loại media không giao việc qua Telegram được.",
                ));
            }
            if project.missing {
                return Ok(json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Thư mục dự án không còn.",
                ));
            }
            if !already {
                if telegram_count(&data) >= crate::telegram::intake::MAX_OFFERS {
                    return Ok(json_error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        &format!(
                            "Tối đa {} dự án cho Telegram.",
                            crate::telegram::intake::MAX_OFFERS
                        ),
                    ));
                }
                match data
                    .get_mut("telegram_projects")
                    .and_then(|v| v.as_array_mut())
                {
                    Some(arr) => arr.push(serde_json::Value::String(id.clone())),
                    None => data["telegram_projects"] = serde_json::json!([id.clone()]),
                }
            }
        } else if let Some(arr) = data
            .get_mut("telegram_projects")
            .and_then(|v| v.as_array_mut())
        {
            arr.retain(|v| v.as_str() != Some(id.as_str()));
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn update_scan_roots(
    State(st): State<Arc<AppState>>,
    Json(body): Json<UpdateScanRootsBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if body.etag != current_etag {
            return conflict_response(&st, current_etag);
        }

        let trimmed: Vec<&str> = body
            .roots
            .iter()
            .map(|r| r.trim())
            .filter(|r| !r.is_empty())
            .collect();
        if trimmed.len() > MAX_SCAN_ROOTS {
            return Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                &format!("Tối đa {MAX_SCAN_ROOTS} thư mục gốc."),
            ));
        }

        let mut kept: Vec<serde_json::Value> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for raw in trimmed {
            let dir = match parse_dir(raw) {
                Ok(d) => d,
                Err(e) => return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e)),
            };
            let canon = std::fs::canonicalize(&dir).unwrap_or(dir);
            if seen.insert(canon) {
                kept.push(serde_json::Value::String(raw.to_string()));
            }
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        data["scan_roots"] = serde_json::Value::Array(kept);
        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn add_project_api(
    State(st): State<Arc<AppState>>,
    Json(body): Json<AddProjectBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if body.etag != current_etag {
            return conflict_response(&st, current_etag);
        }

        if !matches!(body.kind.as_str(), "code" | "media" | "idea") {
            return Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Loại dự án không hợp lệ — chọn code, idea hoặc media.",
            ));
        }
        let dir = match parse_dir(body.path.trim()) {
            Ok(d) => d,
            Err(e) => return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e)),
        };
        if body.kind == "code" && !crate::gitops::is_repo_root(&dir) {
            return Ok(json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Thư mục không phải repo git — chọn loại idea hoặc chạy git init.",
            ));
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let (entry, is_new) = crate::projects::add_project(&mut data, &dir, Some(&body.kind))?;
        let entry_id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let project_val = crate::projects::projects_of(&data)
            .iter()
            .find(|p| p.id == entry_id)
            .map(|p| project_json(p, &data))
            .unwrap_or(serde_json::Value::Null);
        let status = if is_new {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        };
        save_and_respond(&st, &data, status, Some(("project", project_val)))
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn fs_dirs(Query(query): Query<FsDirsQuery>) -> Response {
    let res = blocking(move || {
        let show_hidden = query.hidden.as_deref() == Some("1");
        let raw = query
            .path
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::config::home_dir().to_string_lossy().to_string());
        let dir = match parse_path(&raw) {
            Ok(d) => d,
            Err(e) => return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e)),
        };
        let canon = match std::fs::canonicalize(&dir) {
            Ok(c) if c.is_dir() => c,
            _ => {
                return Ok(json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &format!("Thư mục không tồn tại: {raw}"),
                ))
            }
        };
        let rd = match std::fs::read_dir(&canon) {
            Ok(r) => r,
            Err(_) => {
                return Ok(json_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    &format!("Không đọc được thư mục: {raw}"),
                ))
            }
        };

        let mut dirs: Vec<(String, bool, bool)> = Vec::new();
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let is_link = meta.file_type().is_symlink();
            let is_dir = if is_link {
                std::fs::metadata(&path)
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            } else {
                meta.is_dir()
            };
            if !is_dir {
                continue;
            }
            let git = !is_link && path.join(".git").exists();
            dirs.push((name, git, is_link));
        }
        dirs.sort_by(|a, b| a.0.cmp(&b.0));
        let truncated = dirs.len() > MAX_DIR_ENTRIES;
        dirs.truncate(MAX_DIR_ENTRIES);

        let items: Vec<serde_json::Value> = dirs
            .into_iter()
            .map(|(name, git, link)| {
                let mut v = serde_json::json!({ "name": name, "git": git });
                if link {
                    v["link"] = serde_json::json!(true);
                }
                v
            })
            .collect();
        let parent = canon
            .parent()
            .map(|p| serde_json::json!(p.to_string_lossy()))
            .unwrap_or(serde_json::Value::Null);

        Ok(Json(serde_json::json!({
            "path": canon.to_string_lossy(),
            "parent": parent,
            "dirs": items,
            "truncated": truncated,
        }))
        .into_response())
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
