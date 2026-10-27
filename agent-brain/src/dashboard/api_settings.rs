//! API Cài đặt (06-06): đội hình, quy trình, nhân sự (ghi an toàn, D-10) và danh sách
//! engine chỉ đọc (D-08). Mọi route nằm trong `auth::gate` và giới hạn body 64 KiB.

use crate::dashboard::settings_store::{self, WriteError};
use crate::dashboard::{blocking, json_error, AppState};
use crate::error::BrainResult;
use crate::staff::LoadedStaff;
use crate::teams::{FieldError, LoadedTeams};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

const TEAMS_HEADING: &str = "Không lưu được đội hình";
const TEAMS_CONFLICT: &str = "teams.json vừa bị sửa ở nơi khác";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/teams", get(get_teams))
        .route("/api/teams/save", post(save_team))
        .route("/api/teams/delete", post(delete_team))
        .route("/api/staff", get(get_staff))
        .route("/api/staff/save", post(save_staff))
        .route("/api/staff/delete", post(delete_staff))
}

// ---- shared response shapes ----

fn json_response(status: StatusCode, body: Value) -> Response {
    (
        status,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        Json(body),
    )
        .into_response()
}

fn field_err(field: &str, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.to_string(),
        message: message.into(),
    }
}

/// 422 `{error, errors: [{field, message}]}`.
fn refusal(heading: &str, errors: Vec<FieldError>) -> Response {
    json_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": heading, "errors": errors }),
    )
}

/// 409 `{error, code: "conflict"}`.
fn conflict(message: &str) -> Response {
    json_response(
        StatusCode::CONFLICT,
        json!({ "error": message, "code": "conflict" }),
    )
}

fn invalid_file_text(file: &str) -> String {
    format!("{file} đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi.")
}

/// 409 `{error, code: "invalid_file", errors}`: a file that fails to load is never overwritten.
fn invalid_file(file: &str, errors: &[String]) -> Response {
    json_response(
        StatusCode::CONFLICT,
        json!({ "error": invalid_file_text(file), "code": "invalid_file", "errors": errors }),
    )
}

/// The detail goes to the terminal only; the client gets a fixed text.
fn internal_save(detail: &str) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ khi lưu cài đặt: {detail}"));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Lỗi nội bộ khi lưu cài đặt",
    )
}

fn internal_read(detail: &str) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ khi đọc cài đặt: {detail}"));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Lỗi nội bộ khi đọc cài đặt",
    )
}

fn write_error_response(e: WriteError, conflict_msg: &str, heading: &str) -> Response {
    match e {
        WriteError::Conflict => conflict(conflict_msg),
        WriteError::Refused(errs) => refusal(heading, errs),
        WriteError::Io(detail) => internal_save(&detail),
    }
}

/// `{ok: true, ...payload}`.
fn with_ok(payload: Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("ok".to_string(), Value::Bool(true));
    if let Value::Object(m) = payload {
        out.extend(m);
    }
    Value::Object(out)
}

fn builtin_names(json_text: &str) -> Vec<String> {
    serde_json::from_str::<Value>(json_text)
        .ok()
        .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default()
}

fn project_display_name(p: &crate::projects::Project) -> String {
    if p.name_vi.trim().is_empty() {
        p.id.clone()
    } else {
        p.name_vi.clone()
    }
}

// ---- teams payload ----

fn open_task_count(conn: &Connection, team: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status != 'qa_pass' \
         AND CASE WHEN json_valid(team_json) THEN json_extract(team_json, '$.name') END = ?1",
        [team],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0)
}

pub fn teams_json(
    teams: &LoadedTeams,
    staff: &LoadedStaff,
    map: &Value,
    conn: Option<&Connection>,
    version: &str,
) -> Value {
    let projects = crate::projects::projects_of(map);

    let list: Vec<Value> = teams
        .teams
        .iter()
        .map(|t| {
            let mut default_for = Vec::new();
            let mut used_projects = Vec::new();
            for p in &projects {
                let explicit = crate::projects::team_of(map, &p.id);
                let effective = match crate::teams::resolve(teams, None, explicit.as_deref()) {
                    Ok(r) => r.team.name,
                    Err(_) => crate::teams::DEFAULT_TEAM.to_string(),
                };
                if effective == t.name {
                    default_for.push(project_display_name(p));
                }
                if explicit.as_deref() == Some(t.name.as_str()) {
                    used_projects.push(p.id.clone());
                }
            }
            let open_tasks = conn.map(|c| open_task_count(c, &t.name)).unwrap_or(0);
            let role = |r: &str| t.engine_for(r).to_config_string();
            json!({
                "name": t.name,
                "source": t.source,
                "description": t.description,
                "roles": {
                    "lead": role("lead"),
                    "dev": role("dev"),
                    "qa": role("qa"),
                    "plan_review": role("plan-review"),
                    "code_review": role("code-review"),
                },
                "summary": t.summary(staff),
                "warnings": t.warnings,
                "default_for": default_for,
                "used_by": { "projects": used_projects, "open_tasks": open_tasks },
            })
        })
        .collect();

    json!({
        "teams": list,
        "errors": teams.errors,
        "version": version,
        "builtin_names": builtin_names(crate::teams::BUILTIN_TEAMS_JSON),
    })
}

fn teams_payload(st: &AppState) -> BrainResult<Value> {
    let staff = crate::staff::load();
    let (text, version) = settings_store::read_doc(&crate::teams::teams_path())?;
    let teams = crate::teams::load_from(text.as_deref(), &staff);
    let map = crate::projects::load_map(&st.map_path)
        .unwrap_or_else(|_| json!({"scan_roots": [], "projects": []}));
    let conn = crate::db::open(&st.db_path).ok();
    Ok(teams_json(&teams, &staff, &map, conn.as_ref(), &version))
}

async fn get_teams(State(st): State<Arc<AppState>>) -> Response {
    match blocking(move || teams_payload(&st)).await {
        Ok(v) => json_response(StatusCode::OK, v),
        Err(e) => internal_read(&e.message),
    }
}

// ---- teams save ----

fn claude_str() -> String {
    "claude".to_string()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamRolesBody {
    #[serde(default = "claude_str")]
    lead: String,
    #[serde(default = "claude_str")]
    dev: String,
    #[serde(default = "claude_str")]
    qa: String,
    #[serde(default = "claude_str")]
    plan_review: String,
    #[serde(default = "claude_str")]
    code_review: String,
}

impl Default for TeamRolesBody {
    fn default() -> Self {
        TeamRolesBody {
            lead: claude_str(),
            dev: claude_str(),
            qa: claude_str(),
            plan_review: claude_str(),
            code_review: claude_str(),
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamEntryBody {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    roles: TeamRolesBody,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamSaveBody {
    version: String,
    create: bool,
    entry: TeamEntryBody,
}

/// The UI entry as the loader reads it (underscored role keys become the file's hyphens).
fn team_file_entry(entry: &TeamEntryBody) -> Value {
    let r = &entry.roles;
    json!({
        "description": entry.description.trim(),
        "roles": {
            "lead": r.lead,
            "dev": r.dev,
            "qa": r.qa,
            "plan-review": r.plan_review,
            "code-review": r.code_review,
        }
    })
}

const READONLY_TEAM: &str = "Đội hình có sẵn chỉ đọc — bấm Nhân bản để tạo bản riêng.";

/// `Some(message)` when the name rules refuse the write. A name that exists only among
/// the built-ins is read-only; a name present in the file (`file` or `override`) can be
/// updated or deleted; names are immutable, so an update never renames.
fn team_name_rule(loaded: &LoadedTeams, name: &str, create: bool) -> Option<String> {
    match (create, loaded.get(name).map(|t| t.source.as_str())) {
        (true, Some("builtin")) => Some(format!(
            "'{name}' là đội hình có sẵn, không ghi đè được. Đặt tên khác."
        )),
        (true, Some(_)) => Some(format!("Đã có đội hình tên '{name}'.")),
        (true, None) | (false, Some("file")) | (false, Some("override")) => None,
        (false, Some(_)) => Some(READONLY_TEAM.to_string()),
        (false, None) => Some(format!("Không có đội hình '{name}' trong teams.json.")),
    }
}

/// Loader field names for the hyphenated roles, spelled as the request spells them.
fn ui_field(mut e: FieldError) -> FieldError {
    e.field = e.field.replace('-', "_");
    e
}

/// Runs a write handler body on the blocking pool.
async fn run_write<F>(f: F) -> Response
where
    F: FnOnce() -> Response + Send + 'static,
{
    match blocking(move || Ok(f())).await {
        Ok(r) => r,
        Err(e) => internal_save(&e.message),
    }
}

/// Reads a settings file for a write; a read failure is already a 500 response.
fn read_for_write(path: &Path) -> Result<(Option<String>, String), Response> {
    settings_store::read_doc(path).map_err(|e| internal_save(&e.message))
}

async fn save_team(State(st): State<Arc<AppState>>, Json(body): Json<TeamSaveBody>) -> Response {
    run_write(move || save_team_blocking(&st, body)).await
}

fn save_team_blocking(st: &AppState, body: TeamSaveBody) -> Response {
    let path = crate::teams::teams_path();
    let staff = crate::staff::load();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = crate::teams::load_from(text.as_deref(), &staff);
    if !loaded.errors.is_empty() {
        return invalid_file("teams.json", &loaded.errors);
    }
    if version != body.version {
        return conflict(TEAMS_CONFLICT);
    }

    let name = body.entry.name.clone();
    if let Some(msg) = team_name_rule(&loaded, &name, body.create) {
        return refusal(TEAMS_HEADING, vec![field_err("name", msg)]);
    }
    let file_entry =
        match crate::teams::validate_entry(&name, &team_file_entry(&body.entry), &staff, "file") {
            Ok(team) => team.to_file_entry(),
            Err(errs) => {
                return refusal(TEAMS_HEADING, errs.into_iter().map(ui_field).collect());
            }
        };

    if let Err(e) = settings_store::write_object(&path, &body.version, |m| {
        m.insert(name.clone(), file_entry);
        Ok(())
    }) {
        return write_error_response(e, TEAMS_CONFLICT, TEAMS_HEADING);
    }

    match teams_payload(st) {
        Ok(v) => json_response(StatusCode::OK, with_ok(v)),
        Err(e) => internal_save(&e.message),
    }
}

// ---- teams delete ----

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamDeleteBody {
    version: String,
    name: String,
}

async fn delete_team(
    State(st): State<Arc<AppState>>,
    Json(body): Json<TeamDeleteBody>,
) -> Response {
    run_write(move || delete_team_blocking(&st, body)).await
}

fn delete_team_blocking(st: &AppState, body: TeamDeleteBody) -> Response {
    let path = crate::teams::teams_path();
    let staff = crate::staff::load();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = crate::teams::load_from(text.as_deref(), &staff);
    if !loaded.errors.is_empty() {
        return invalid_file("teams.json", &loaded.errors);
    }
    if version != body.version {
        return conflict(TEAMS_CONFLICT);
    }

    let name = body.name.clone();
    if let Some(msg) = team_name_rule(&loaded, &name, false) {
        return refusal(TEAMS_HEADING, vec![field_err("name", msg)]);
    }

    if let Err(e) = settings_store::write_object(&path, &body.version, |m| {
        m.shift_remove(&name);
        Ok(())
    }) {
        return write_error_response(e, TEAMS_CONFLICT, TEAMS_HEADING);
    }

    match teams_payload(st) {
        Ok(v) => json_response(StatusCode::OK, with_ok(v)),
        Err(e) => internal_save(&e.message),
    }
}

// ---- staff ----

const STAFF_HEADING: &str = "Không lưu được nhân sự";
const STAFF_CONFLICT: &str = "file nhân sự vừa bị sửa ở nơi khác";

pub fn staff_json(staff: &LoadedStaff, teams: &LoadedTeams, version: &str) -> Value {
    let list: Vec<Value> = staff
        .staff
        .iter()
        .map(|s| {
            let used_in: Vec<&str> = teams
                .teams
                .iter()
                .filter(|t| t.uses_staff(&s.id))
                .map(|t| t.name.as_str())
                .collect();
            json!({
                "id": s.id,
                "display_name": s.display_name,
                "notes": s.notes.clone().unwrap_or_default(),
                "used_in": used_in,
            })
        })
        .collect();
    json!({ "staff": list, "errors": staff.errors, "version": version })
}

fn staff_payload() -> BrainResult<Value> {
    let (text, version) = settings_store::read_doc(&crate::staff::staff_path())?;
    let staff = crate::staff::load_from(text.as_deref());
    let teams = crate::teams::load_path(&crate::teams::teams_path(), &staff);
    Ok(staff_json(&staff, &teams, &version))
}

async fn get_staff() -> Response {
    match blocking(staff_payload).await {
        Ok(v) => json_response(StatusCode::OK, v),
        Err(e) => internal_read(&e.message),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StaffEntryBody {
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    notes: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StaffSaveBody {
    version: String,
    create: bool,
    entry: StaffEntryBody,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StaffDeleteBody {
    version: String,
    id: String,
}

fn staff_rule(loaded: &LoadedStaff, id: &str, create: bool) -> Option<String> {
    match (create, loaded.get(id).is_some()) {
        (true, true) => Some(format!("Đã có nhân sự mã '{id}'.")),
        (false, false) => Some(format!("Không có nhân sự '{id}' trong staff.json.")),
        _ => None,
    }
}

async fn save_staff(Json(body): Json<StaffSaveBody>) -> Response {
    run_write(move || save_staff_blocking(body)).await
}

fn save_staff_blocking(body: StaffSaveBody) -> Response {
    let path = crate::staff::staff_path();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = crate::staff::load_from(text.as_deref());
    if !loaded.errors.is_empty() {
        return invalid_file("staff.json", &loaded.errors);
    }
    if version != body.version {
        return conflict(STAFF_CONFLICT);
    }

    let id = body.entry.id.clone();
    if let Some(msg) = staff_rule(&loaded, &id, body.create) {
        return refusal(STAFF_HEADING, vec![field_err("id", msg)]);
    }

    let mut entry = serde_json::Map::new();
    entry.insert(
        "display_name".to_string(),
        Value::String(body.entry.display_name.trim().to_string()),
    );
    let notes = body.entry.notes.trim();
    if !notes.is_empty() {
        entry.insert("notes".to_string(), Value::String(notes.to_string()));
    }
    let file_entry = match crate::staff::validate_entry(&id, &Value::Object(entry)) {
        Ok(s) => {
            let mut m = serde_json::Map::new();
            m.insert("display_name".to_string(), Value::String(s.display_name));
            if let Some(n) = s.notes {
                m.insert("notes".to_string(), Value::String(n));
            }
            Value::Object(m)
        }
        Err(errs) => return refusal(STAFF_HEADING, errs),
    };

    if let Err(e) = settings_store::write_object(&path, &body.version, |m| {
        m.insert(id.clone(), file_entry);
        Ok(())
    }) {
        return write_error_response(e, STAFF_CONFLICT, STAFF_HEADING);
    }

    match staff_payload() {
        Ok(v) => json_response(StatusCode::OK, with_ok(v)),
        Err(e) => internal_save(&e.message),
    }
}

async fn delete_staff(Json(body): Json<StaffDeleteBody>) -> Response {
    run_write(move || delete_staff_blocking(body)).await
}

fn delete_staff_blocking(body: StaffDeleteBody) -> Response {
    let path = crate::staff::staff_path();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = crate::staff::load_from(text.as_deref());
    if !loaded.errors.is_empty() {
        return invalid_file("staff.json", &loaded.errors);
    }
    if version != body.version {
        return conflict(STAFF_CONFLICT);
    }

    let id = body.id.clone();
    if let Some(msg) = staff_rule(&loaded, &id, false) {
        return refusal(STAFF_HEADING, vec![field_err("id", msg)]);
    }
    let teams = crate::teams::load_path(&crate::teams::teams_path(), &loaded);
    let users: Vec<&str> = teams
        .teams
        .iter()
        .filter(|t| t.uses_staff(&id))
        .map(|t| t.name.as_str())
        .collect();
    if !users.is_empty() {
        return refusal(
            STAFF_HEADING,
            vec![field_err(
                "id",
                format!(
                    "Đang dùng trong đội hình: {}. Đổi engine của vai trò đó trước rồi xóa.",
                    users.join(", ")
                ),
            )],
        );
    }

    if let Err(e) = settings_store::write_object(&path, &body.version, |m| {
        m.shift_remove(&id);
        Ok(())
    }) {
        return write_error_response(e, STAFF_CONFLICT, STAFF_HEADING);
    }

    match staff_payload() {
        Ok(v) => json_response(StatusCode::OK, with_ok(v)),
        Err(e) => internal_save(&e.message),
    }
}
