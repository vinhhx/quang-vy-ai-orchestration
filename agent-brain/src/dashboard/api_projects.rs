//! Owning plan: 05-07

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Json;
use axum::Router;
use std::collections::HashSet;
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

type Profiles = std::collections::BTreeMap<String, crate::config::ProfileCfg>;

/// Cheap reason (no git, no shadow) why a project cannot run a task yet.
fn runnable_issue(p: &crate::projects::Project, profiles: &Profiles) -> Option<String> {
    if p.missing {
        return Some("Thư mục dự án không còn — chưa chạy được".to_string());
    }
    if p.kind == "media" {
        return Some("Dự án loại media — không chạy task được".to_string());
    }
    if p.profile.trim().is_empty() {
        return Some("Chưa đặt profile — chưa chạy được".to_string());
    }
    if !profiles.contains_key(&p.profile) {
        return Some(format!(
            "Profile '{}' không hợp lệ — chưa chạy được",
            p.profile
        ));
    }
    None
}

fn project_json(
    p: &crate::projects::Project,
    map: &serde_json::Value,
    profiles: &Profiles,
) -> serde_json::Value {
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
        "stack": p.stack,
        "read_sources": p.read_sources,
        "workflow": crate::projects::workflow_of(map, &p.id),
        "timeouts": crate::projects::timeouts_of(map, &p.id),
        "protocol_dir": p.protocol_dir,
        "base_branch": p.workspace.base_branch,
        "hold_project_at_gate": p.workspace.hold_project_at_gate,
        "auto_checkout_on_approve": p.workspace.auto_checkout_on_approve,
        "workspace": p.workspace.mode.as_str(),
        "max_parallel_tasks": p.workspace.max_parallel_tasks,
        "role_members": crate::projects::role_pins_of(map, &p.id).members,
        "role_engines": crate::projects::role_pins_of(map, &p.id).engines,
        "preflight": preflight_json(map, &p.id),
        "runnable_issue": runnable_issue(p, profiles),
    })
}

fn projects_body(map: &serde_json::Value, etag: String, profiles: &Profiles) -> serde_json::Value {
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
        .map(|p| project_json(&p, map, profiles))
        .collect();
    serde_json::json!({
        "etag": etag,
        "scan_roots": scan_roots,
        "projects": projects_json,
        "profiles": profiles.keys().cloned().collect::<Vec<_>>(),
        "duplicates": duplicates,
        "telegram_count": telegram_count(map),
        "telegram_max": crate::telegram::intake::MAX_OFFERS,
    })
}

fn conflict_response(st: &AppState, current_etag: String) -> crate::error::BrainResult<Response> {
    let map = crate::projects::load_map(&st.map_path)?;
    let mut body = projects_body(&map, current_etag, &st.cfg.profiles);
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
    save_and_respond_with(st, data, status, extra, Vec::new())
}

fn save_and_respond_with(
    st: &AppState,
    data: &serde_json::Value,
    status: StatusCode,
    extra: Option<(&str, serde_json::Value)>,
    extra_warnings: Vec<String>,
) -> crate::error::BrainResult<Response> {
    crate::projects::save_map(&st.map_path, data)?;
    if let Ok(conn) = crate::db::open(&st.db_path) {
        crate::db::sync_projects(&conn, &crate::projects::projects_of(data))?;
    }
    let new_etag = map_etag(&st.map_path);
    let mut body = projects_body(data, new_etag, &st.cfg.profiles);
    let raw_projects = data
        .get("projects")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let mut warnings: Vec<String> = crate::projects::find_duplicates(&raw_projects)
        .iter()
        .map(|(key, ids)| format!("CẢNH BÁO: tên/alias trùng \"{key}\": {}", ids.join(", ")))
        .collect();
    warnings.extend(extra_warnings);
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
    #[serde(default)]
    pub profile: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfileBody {
    pub profile: String,
    pub etag: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateTimeoutsBody {
    /// `{role: seconds}`; an empty object removes the project's `timeouts`.
    pub timeouts: serde_json::Value,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateProtocolDirBody {
    /// Folder of the protocol files relative to the repo root; `""` removes the key (root).
    pub protocol_dir: String,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateWorkspaceBody {
    /// Branch new tasks start from; `""` removes the key (the current branch at creation),
    /// absent keeps the stored value.
    #[serde(default)]
    pub base_branch: Option<String>,
    /// `hold_project_at_gate`; absent keeps the stored value.
    #[serde(default)]
    pub hold_project_at_gate: Option<bool>,
    /// `auto_checkout_on_approve`; absent keeps the stored value.
    #[serde(default)]
    pub auto_checkout_on_approve: Option<bool>,
    /// `workspace`: `"shared"` or `"worktree"`; absent keeps the stored value.
    #[serde(default)]
    pub workspace: Option<String>,
    /// `max_parallel_tasks` (`projects::MAX_PARALLEL_RANGE`); absent keeps the stored value.
    #[serde(default)]
    pub max_parallel_tasks: Option<i64>,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateRolePinsBody {
    /// `{role: member id}`; `{}` (or absent) removes the project's `role_members`.
    #[serde(default)]
    pub role_members: serde_json::Value,
    /// `{role: engine name}`; `{}` (or absent) removes the project's `role_engines`.
    #[serde(default)]
    pub role_engines: serde_json::Value,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateStackBody {
    pub stack: Vec<String>,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateReadSourcesBody {
    pub read_sources: Vec<String>,
    #[serde(default)]
    pub etag: Option<String>,
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
        .route("/api/projects/{id}/profile", put(update_profile))
        .route("/api/projects/{id}/stack", put(update_stack))
        .route("/api/projects/{id}/timeouts", put(update_timeouts))
        .route("/api/projects/{id}/protocol-dir", put(update_protocol_dir))
        .route("/api/projects/{id}/workspace", put(update_workspace))
        .route("/api/projects/{id}/role-pins", put(update_role_pins))
        .route("/api/projects/{id}/preflight", put(update_preflight))
        .route("/api/projects/{id}/read-sources", post(update_read_sources).put(update_read_sources))
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
        let mut body = projects_body(&map, etag, &st.cfg.profiles);
        if let Some(prof) = profile_opt {
            projects.retain(|p| p.profile == prof);
            let filtered: Vec<serde_json::Value> = projects
                .iter()
                .map(|p| project_json(p, &map, &st.cfg.profiles))
                .collect();
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

async fn update_profile(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateProfileBody>,
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
        let project_obj = match projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
        {
            Some(obj) => obj,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        let profile = body.profile.trim().to_string();
        if let Err(e) = crate::profile::check_profile(&profile, &st.cfg.profiles) {
            return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e.message));
        }
        project_obj["profile"] = serde_json::Value::String(profile);

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn update_stack(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateStackBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(ref req_etag) = body.etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        if body.stack.len() > 6 {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                "Tối đa 6 stack",
            ));
        }

        let mut cleaned = Vec::new();
        for s in body.stack {
            let trimmed = s.trim().to_string();
            if !crate::prompts::valid_name(&trimmed) {
                return Ok(json_error(
                    StatusCode::BAD_REQUEST,
                    &format!("Tên stack '{trimmed}' không hợp lệ"),
                ));
            }
            if !cleaned.contains(&trimmed) {
                cleaned.push(trimmed);
            }
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let project_obj = match projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
        {
            Some(obj) => obj,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        project_obj["stack"] = serde_json::Value::Array(
            cleaned
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

/// C1: the project's `timeouts: {role: seconds}`, validated like workflows.json
/// (`config::parse_role_timeouts`). `{}` removes the key.
async fn update_timeouts(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateTimeoutsBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(ref req_etag) = body.etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        let timeouts = match crate::config::parse_role_timeouts(&body.timeouts) {
            Ok(t) => t,
            Err(errs) => {
                return Ok(json_error(StatusCode::BAD_REQUEST, &errs.join(" ")));
            }
        };

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let Some(project_obj) = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
            .and_then(|obj| obj.as_object_mut())
        else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        };

        if timeouts.is_empty() {
            project_obj.remove("timeouts");
        } else {
            project_obj.insert("timeouts".to_string(), serde_json::json!(timeouts));
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

/// D1: the project's `protocol_dir`, validated by `protocol_paths::validate_protocol_dir`
/// (relative, no `..`, not inside `.git`, `{slug}` the only variable). `""` removes the key.
async fn update_protocol_dir(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateProtocolDirBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(ref req_etag) = body.etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        let dir = match crate::protocol_paths::validate_protocol_dir(&body.protocol_dir) {
            Ok(d) => d,
            Err(e) => return Ok(json_error(StatusCode::BAD_REQUEST, &e)),
        };

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let Some(project_obj) = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
            .and_then(|obj| obj.as_object_mut())
        else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        };

        if dir.is_empty() {
            project_obj.remove(crate::protocol_paths::CONFIG_KEY);
        } else {
            project_obj.insert(
                crate::protocol_paths::CONFIG_KEY.to_string(),
                serde_json::json!(dir),
            );
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

/// D2: the project's workspace options. `base_branch` is validated by
/// `projects::validate_base_branch` (`""` removes it); a boolean equal to its default removes
/// the key, so a project left at the defaults keeps no extra keys.
async fn update_workspace(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateWorkspaceBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(ref req_etag) = body.etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        let base = match body
            .base_branch
            .as_deref()
            .map(crate::projects::validate_base_branch)
            .transpose()
        {
            Ok(b) => b,
            Err(e) => return Ok(json_error(StatusCode::BAD_REQUEST, &e)),
        };

        let mode = match body.workspace.as_deref() {
            None => None,
            Some(raw) => match crate::projects::WorkspaceMode::parse(raw) {
                Some(m) => Some(m),
                None => {
                    return Ok(json_error(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "\"{}\" chỉ nhận \"shared\" hoặc \"worktree\".",
                            crate::projects::WORKSPACE_KEY
                        ),
                    ))
                }
            },
        };
        let max_parallel = match body.max_parallel_tasks {
            None => None,
            Some(n) if n >= 0 && crate::projects::MAX_PARALLEL_RANGE.contains(&(n as usize)) => {
                Some(n as usize)
            }
            Some(_) => {
                return Ok(json_error(
                    StatusCode::BAD_REQUEST,
                    &format!(
                        "\"{}\" phải từ {} đến {}.",
                        crate::projects::MAX_PARALLEL_KEY,
                        crate::projects::MAX_PARALLEL_RANGE.start(),
                        crate::projects::MAX_PARALLEL_RANGE.end()
                    ),
                ))
            }
        };

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let Some(project_obj) = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
            .and_then(|obj| obj.as_object_mut())
        else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        };

        // A value equal to its default removes the key, as for the booleans below.
        let defaults = crate::projects::WorkspaceOpts::default();
        // The default mode of this project: `worktree` only for a git code project (a project
        // that cannot have worktrees defaults to `shared`, so picking `worktree` there is stored).
        let default_mode = crate::projects::default_mode_for(&serde_json::Value::Object(
            project_obj.clone(),
        ));
        match mode {
            Some(m) if m == default_mode => {
                project_obj.remove(crate::projects::WORKSPACE_KEY);
            }
            Some(m) => {
                project_obj.insert(
                    crate::projects::WORKSPACE_KEY.to_string(),
                    serde_json::json!(m.as_str()),
                );
            }
            None => {}
        }
        match max_parallel {
            Some(n) if n == defaults.max_parallel_tasks => {
                project_obj.remove(crate::projects::MAX_PARALLEL_KEY);
            }
            Some(n) => {
                project_obj.insert(
                    crate::projects::MAX_PARALLEL_KEY.to_string(),
                    serde_json::json!(n),
                );
            }
            None => {}
        }

        match base.as_deref() {
            Some("") => {
                project_obj.remove(crate::projects::BASE_BRANCH_KEY);
            }
            Some(b) => {
                project_obj.insert(
                    crate::projects::BASE_BRANCH_KEY.to_string(),
                    serde_json::json!(b),
                );
            }
            None => {}
        }
        for (key, value, default) in [
            (
                crate::projects::HOLD_AT_GATE_KEY,
                body.hold_project_at_gate,
                defaults.hold_project_at_gate,
            ),
            (
                crate::projects::AUTO_CHECKOUT_KEY,
                body.auto_checkout_on_approve,
                defaults.auto_checkout_on_approve,
            ),
        ] {
            match value {
                Some(v) if v == default => {
                    project_obj.remove(key);
                }
                Some(v) => {
                    project_obj.insert(key.to_string(), serde_json::json!(v));
                }
                None => {}
            }
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

/// C4: the project's role pins `role_members` / `role_engines`, validated by
/// `projects::parse_role_pins`. A pinned member must exist in departments.json and a pinned engine
/// in config.json; whether it is ready is checked when a run starts. `{}` removes a key.
async fn update_role_pins(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<UpdateRolePinsBody>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(ref req_etag) = body.etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        let pins = match crate::projects::parse_role_pins(&body.role_members, &body.role_engines) {
            Ok(p) => p,
            Err(errs) => {
                return Ok(json_error(StatusCode::BAD_REQUEST, &errs.join(" ")));
            }
        };
        let mut errors = Vec::new();
        if !pins.members.is_empty() {
            let departments =
                crate::departments::load_path(&crate::departments::departments_path(), &st.cfg);
            for (role, member) in &pins.members {
                if departments.get(member).is_none() {
                    errors.push(format!(
                        "Vai {role}: thành viên '{member}' không có trong departments.json."
                    ));
                }
            }
        }
        for (role, engine) in &pins.engines {
            if let Some(crate::engine::Engine::Named(eid)) = crate::engine::Engine::parse(engine) {
                if st.cfg.engines.get(&eid).is_none() {
                    errors.push(format!(
                        "Vai {role}: engine '{eid}' không có trong config.json (engines)."
                    ));
                }
            }
        }
        if !errors.is_empty() {
            return Ok(json_error(StatusCode::BAD_REQUEST, &errors.join(" ")));
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let Some(project_obj) = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
            .and_then(|obj| obj.as_object_mut())
        else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        };

        for (key, values) in [("role_members", &pins.members), ("role_engines", &pins.engines)] {
            if values.is_empty() {
                project_obj.remove(key);
            } else {
                project_obj.insert(key.to_string(), serde_json::json!(values));
            }
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

/// C2: the project's preflight settings as the dashboard edits them (defaults filled in).
fn preflight_json(map: &serde_json::Value, project_id: &str) -> serde_json::Value {
    let pf = crate::projects::preflight_of(map, project_id);
    let stored = crate::preflight::to_json_fields(&pf);
    serde_json::json!({
        "checks": stored.get("preflight").cloned().unwrap_or_else(|| serde_json::json!([])),
        "roles": pf.roles,
        "retry_minutes": pf.retry_minutes,
        "interval_seconds": pf.interval_seconds,
    })
}

/// The project keys `update_preflight` owns.
const PREFLIGHT_KEYS: [&str; 4] = [
    "preflight",
    "preflight_roles",
    "preflight_retry_minutes",
    "preflight_interval_seconds",
];

/// C2: `{preflight, preflight_roles, preflight_retry_minutes, preflight_interval_seconds, etag}`,
/// validated by `preflight::parse_settings`. Keys at their default are removed, so an empty
/// `preflight` list puts the project back to no check at all.
async fn update_preflight(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        if let Some(req_etag) = body.get("etag").and_then(|e| e.as_str()) {
            if req_etag != current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        let settings = match crate::preflight::parse_settings(&body) {
            Ok(p) => p,
            Err(errs) => {
                return Ok(json_error(StatusCode::BAD_REQUEST, &errs.join(" ")));
            }
        };

        let mut data = crate::projects::load_map(&st.map_path)?;
        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let Some(project_obj) = projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
            .and_then(|obj| obj.as_object_mut())
        else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        };

        for k in PREFLIGHT_KEYS {
            project_obj.remove(k);
        }
        for (k, v) in crate::preflight::to_json_fields(&settings) {
            project_obj.insert(k, v);
        }

        save_and_respond(&st, &data, StatusCode::OK, None)
    })
    .await;

    match res {
        Ok(r) => r,
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}

async fn update_read_sources(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateReadSourcesBody>,
) -> Response {
    let if_match = headers
        .get(axum::http::header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string());
    let res = blocking(move || {
        let _guard = write_mutex().lock().unwrap();

        let current_etag = map_etag(&st.map_path);
        let check_etag = body.etag.or(if_match);
        if let Some(ref req_etag) = check_etag {
            if req_etag != &current_etag {
                return conflict_response(&st, current_etag);
            }
        }

        if body.read_sources.len() > 5 {
            return Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                Json(serde_json::json!({
                    "error": "Tối đa 5 nguồn",
                    "errors": [{ "field": "read_sources", "message": "Tối đa 5 nguồn" }]
                })),
            )
                .into_response());
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let all_projects = crate::projects::projects_of(&data);
        if !all_projects.iter().any(|p| p.id == id) {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {id}"),
            ));
        }

        let mut cleaned = Vec::new();
        let mut seen = HashSet::new();
        for s in body.read_sources {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == id {
                return Ok((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": "Không thể tự thêm chính dự án làm nguồn",
                        "errors": [{ "field": "read_sources", "message": "Không thể tự thêm chính dự án làm nguồn" }]
                    })),
                )
                    .into_response());
            }
            if !all_projects.iter().any(|p| p.id == trimmed) {
                return Ok((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": format!("Dự án nguồn '{trimmed}' không tồn tại"),
                        "errors": [{ "field": "read_sources", "message": format!("Dự án nguồn '{trimmed}' không tồn tại") }]
                    })),
                )
                    .into_response());
            }
            if !seen.insert(trimmed.clone()) {
                return Ok((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "application/json; charset=utf-8",
                    )],
                    Json(serde_json::json!({
                        "error": format!("Dự án nguồn '{trimmed}' bị trùng lặp"),
                        "errors": [{ "field": "read_sources", "message": format!("Dự án nguồn '{trimmed}' bị trùng lặp") }]
                    })),
                )
                    .into_response());
            }
            cleaned.push(trimmed);
        }

        let projects_arr = data
            .get_mut("projects")
            .and_then(|p| p.as_array_mut())
            .ok_or_else(|| {
                crate::error::BrainError::error("projects_map.json thiếu mảng 'projects'")
            })?;
        let project_obj = match projects_arr
            .iter_mut()
            .find(|obj| obj.get("id").and_then(|v| v.as_str()) == Some(&id))
        {
            Some(obj) => obj,
            None => {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    &format!("Không tìm thấy dự án {id}"),
                ))
            }
        };

        project_obj["read_sources"] = serde_json::Value::Array(
            cleaned
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

        let profile = body
            .profile
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if let Some(p) = &profile {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return Ok(json_error(StatusCode::UNPROCESSABLE_ENTITY, &e.message));
            }
        }

        let mut data = crate::projects::load_map(&st.map_path)?;
        let (entry, is_new) = crate::projects::add_project(&mut data, &dir, Some(&body.kind))?;
        let entry_id = entry
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(p) = &profile {
            if let Some(obj) = data
                .get_mut("projects")
                .and_then(|a| a.as_array_mut())
                .and_then(|a| {
                    a.iter_mut()
                        .find(|o| o.get("id").and_then(|v| v.as_str()) == Some(entry_id.as_str()))
                })
            {
                let empty = obj
                    .get("profile")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true);
                if is_new || empty {
                    obj["profile"] = serde_json::Value::String(p.clone());
                }
            }
        }
        let project_val = crate::projects::projects_of(&data)
            .iter()
            .find(|p| p.id == entry_id)
            .map(|p| project_json(p, &data, &st.cfg.profiles))
            .unwrap_or(serde_json::Value::Null);
        let status = if is_new {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        };
        let profile_missing = crate::projects::projects_of(&data)
            .iter()
            .find(|p| p.id == entry_id)
            .map(|p| p.profile.trim().is_empty())
            .unwrap_or(false);
        let extra_warnings = if profile_missing {
            vec!["Dự án chưa có profile — đặt profile trước khi chạy task.".to_string()]
        } else {
            Vec::new()
        };
        save_and_respond_with(
            &st,
            &data,
            status,
            Some(("project", project_val)),
            extra_warnings,
        )
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
        let j = project_json(&projs[0], &map, &Profiles::new());
        assert_eq!(j["workflow"], "nhanh");
    }
}
