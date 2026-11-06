//! API phòng ban và thành viên (departments.json).
//!
//! GET /api/departments
//! POST /api/departments/init-example
//! POST /api/departments/member/save
//! POST /api/departments/member/delete
//! GET /api/departments/models

use crate::dashboard::settings_store::{self, WriteError};
use crate::dashboard::{blocking, json_error, AppState};
use crate::departments::{self, dept_name_of, dept_roles, Level, Member, DEPARTMENTS};
use crate::error::BrainResult;
use crate::teams::FieldError;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEPARTMENTS_HEADING: &str = "Không lưu được thành viên phòng ban";
const DEPARTMENTS_CONFLICT: &str = "departments.json vừa bị sửa ở nơi khác";
const DEPARTMENTS_FILE: &str = "departments.json";
const EXAMPLE_JSON: &str = include_str!("../../assets/departments.example.json");

static AGY_CACHE: Mutex<Option<(Instant, String, Vec<String>)>> = Mutex::new(None);

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/departments", get(get_departments))
        .route("/api/departments/init-example", post(init_example))
        .route("/api/departments/member/save", post(save_member))
        .route("/api/departments/member/delete", post(delete_member))
        .route("/api/departments/models", get(get_models))
        .route("/api/tasks/{id}/level", post(post_task_level))
        .route("/api/proposals/{id}/level", post(post_proposal_level))
        .route("/api/runs/{id}/score", post(post_run_score))
}

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

fn refusal(heading: &str, errors: Vec<FieldError>) -> Response {
    json_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": heading, "errors": errors }),
    )
}

fn conflict(message: &str) -> Response {
    json_response(
        StatusCode::CONFLICT,
        json!({ "error": message, "code": "conflict" }),
    )
}

fn invalid_file(file: &str, errors: &[String]) -> Response {
    let msg = format!("{file} đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi.");
    json_response(
        StatusCode::CONFLICT,
        json!({ "error": msg, "code": "invalid_file", "errors": errors }),
    )
}

fn internal_save(detail: &str) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ khi lưu phòng ban: {detail}"));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Lỗi nội bộ khi lưu phòng ban",
    )
}

fn internal_read(detail: &str) -> Response {
    crate::term::emit(&format!("Lỗi nội bộ khi đọc phòng ban: {detail}"));
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Lỗi nội bộ khi đọc phòng ban",
    )
}

fn write_error_response(e: WriteError, conflict_msg: &str, heading: &str) -> Response {
    match e {
        WriteError::Conflict => conflict(conflict_msg),
        WriteError::Refused(errs) => refusal(heading, errs),
        WriteError::Io(detail) => internal_save(&detail),
    }
}

fn with_ok(payload: Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("ok".to_string(), Value::Bool(true));
    if let Value::Object(m) = payload {
        out.extend(m);
    }
    Value::Object(out)
}

fn live_cfg(st: &AppState) -> crate::config::Config {
    let path = crate::config::config_path();
    if path.is_file() {
        crate::config::load_config().unwrap_or_else(|_| st.cfg.clone())
    } else {
        st.cfg.clone()
    }
}

#[allow(clippy::result_large_err)]
fn read_for_write(path: &Path) -> Result<(Option<String>, String), Response> {
    settings_store::read_doc(path).map_err(|e| internal_save(&e.message))
}

async fn run_write<F>(f: F) -> Response
where
    F: FnOnce() -> Response + Send + 'static,
{
    match blocking(move || Ok(f())).await {
        Ok(r) => r,
        Err(e) => internal_save(&e.message),
    }
}

fn member_stats(conn: &Connection, member_id: &str) -> Value {
    let runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE member_id = ?1",
            rusqlite::params![member_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let succeeded: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE member_id = ?1 AND status = 'succeeded'",
            rusqlite::params![member_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let running: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE member_id = ?1 AND status = 'running'",
            rusqlite::params![member_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let (avg, score_count) = crate::db::member_score_avg(conn, member_id).unwrap_or((0.0, 0));
    let avg_score = if score_count > 0 {
        Some(avg)
    } else {
        None
    };

    json!({
        "runs": runs,
        "succeeded": succeeded,
        "avg_score": avg_score,
        "scores": score_count,
        "running": running,
    })
}

fn member_readiness(
    member: &Member,
    statuses: &[crate::engine::EngineStatus],
) -> (bool, Option<String>) {
    let name = member.engine.as_db_str();
    let status_opt = statuses.iter().find(|s| s.name == name);
    match status_opt {
        Some(s) => {
            let is_ready = s.readiness == "san_sang";
            let reason = if is_ready {
                None
            } else {
                s.reason.clone().or_else(|| Some(s.word.to_string()))
            };
            (is_ready, reason)
        }
        None => (false, Some("Engine không tồn tại trong config".to_string())),
    }
}

pub fn departments_payload(st: &AppState) -> BrainResult<Value> {
    let cfg = live_cfg(st);
    let path = departments::departments_path();
    let file_exists = path.is_file();
    let (text, version) = settings_store::read_doc(&path)?;
    let loaded = departments::load_from(text.as_deref().unwrap_or("{}"), &cfg);

    let env = crate::profile::current_env();
    let staff = crate::staff::load();
    let statuses = crate::engine::readiness(&cfg, &env, staff.staff.len());
    let conn = crate::db::open(&st.db_path)?;

    let mut depts_json = Vec::new();
    for &(dept_id, dept_name) in DEPARTMENTS {
        let roles = dept_roles(dept_id);
        let dept_members = loaded.members_of(dept_id);
        let mut members_json = Vec::new();

        for m in dept_members {
            let (ready, ready_reason) = member_readiness(m, &statuses);
            let stats = member_stats(&conn, &m.id);
            members_json.push(json!({
                "id": m.id,
                "display_name": m.display_name,
                "engine": m.engine.to_config_string(),
                "model": m.model,
                "levels": m.levels.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
                "notes": m.notes,
                "ready": ready,
                "ready_reason": ready_reason,
                "stats": stats,
            }));
        }

        depts_json.push(json!({
            "id": dept_id,
            "name": dept_name,
            "roles": roles,
            "members": members_json,
        }));
    }

    Ok(json!({
        "departments": depts_json,
        "errors": loaded.errors,
        "version": version,
        "file_exists": file_exists,
    }))
}

fn departments_response(st: &AppState) -> Response {
    match departments_payload(st) {
        Ok(v) => json_response(StatusCode::OK, with_ok(v)),
        Err(e) => internal_save(&e.message),
    }
}

async fn get_departments(State(st): State<Arc<AppState>>) -> Response {
    match blocking(move || departments_payload(&st)).await {
        Ok(v) => json_response(StatusCode::OK, v),
        Err(e) => internal_read(&e.message),
    }
}

async fn init_example(State(st): State<Arc<AppState>>) -> Response {
    run_write(move || init_example_blocking(&st)).await
}

fn init_example_blocking(st: &AppState) -> Response {
    let path = departments::departments_path();
    if path.exists() {
        return conflict("File departments.json đã tồn tại, không thể khởi tạo từ mẫu");
    }

    let parsed_example: Value = match serde_json::from_str(EXAMPLE_JSON) {
        Ok(v) => v,
        Err(e) => return internal_save(&format!("Lỗi cú pháp mẫu departments.example.json: {e}")),
    };

    let example_map = match parsed_example.as_object() {
        Some(o) => o.clone(),
        None => return internal_save("departments.example.json không phải là một object"),
    };

    if let Err(e) = settings_store::write_object(&path, settings_store::VERSION_ABSENT, |root| {
        *root = example_map;
        Ok(())
    }) {
        return write_error_response(e, DEPARTMENTS_CONFLICT, DEPARTMENTS_HEADING);
    }

    departments_response(st)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberSaveBody {
    version: String,
    department: String,
    id: String,
    member: Value,
    #[serde(default)]
    old_department: Option<String>,
}

async fn save_member(
    State(st): State<Arc<AppState>>,
    Json(body): Json<MemberSaveBody>,
) -> Response {
    run_write(move || save_member_blocking(&st, body)).await
}

fn save_member_blocking(st: &AppState, body: MemberSaveBody) -> Response {
    let cfg = live_cfg(st);
    let path = departments::departments_path();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = departments::load_from(text.as_deref().unwrap_or("{}"), &cfg);
    if !loaded.errors.is_empty() {
        return invalid_file(DEPARTMENTS_FILE, &loaded.errors);
    }
    if version != body.version {
        return conflict(DEPARTMENTS_CONFLICT);
    }

    let validated = match departments::validate_entry(&body.department, &body.id, &body.member, &cfg) {
        Ok(m) => m,
        Err(errs) => return refusal(DEPARTMENTS_HEADING, errs),
    };

    // Kiểm tra trùng ID giữa các phòng
    if let Some(existing) = loaded.get(&body.id) {
        let is_moving = body
            .old_department
            .as_deref()
            .map(|old| old == existing.department)
            .unwrap_or(false);
        if !is_moving && existing.department != body.department {
            return refusal(
                DEPARTMENTS_HEADING,
                vec![FieldError {
                    field: "id".to_string(),
                    message: format!(
                        "Mã '{}' đã tồn tại ở phòng '{}'",
                        body.id,
                        dept_name_of(&existing.department).unwrap_or(&existing.department)
                    ),
                }],
            );
        }
    }

    let old_dept = body.old_department.clone();
    let dept_id = body.department.clone();
    let member_id = body.id.clone();

    if let Err(e) = settings_store::write_object(&path, &body.version, |root| {
        // Xóa ở phòng cũ nếu có đổi phòng
        if let Some(old_d) = &old_dept {
            if old_d != &dept_id {
                if let Some(old_val) = root.get_mut(old_d) {
                    if let Some(members) = old_val.get_mut("members").and_then(|m| m.as_object_mut()) {
                        members.remove(&member_id);
                    }
                }
            }
        }

        // Tạo hoặc lấy phòng mục tiêu
        let dept_val = root.entry(dept_id.clone()).or_insert_with(|| json!({ "members": {} }));
        let dept_obj = dept_val.as_object_mut().ok_or_else(|| {
            vec![FieldError {
                field: "department".to_string(),
                message: format!("Phòng '{dept_id}' phải là một object"),
            }]
        })?;
        let members_val = dept_obj.entry("members".to_string()).or_insert_with(|| json!({}));
        let members_obj = members_val.as_object_mut().ok_or_else(|| {
            vec![FieldError {
                field: "members".to_string(),
                message: format!("Trường members của phòng '{dept_id}' phải là một object"),
            }]
        })?;

        let mut entry = serde_json::Map::new();
        entry.insert("display_name".to_string(), Value::String(validated.display_name));
        entry.insert("engine".to_string(), Value::String(validated.engine.to_config_string()));
        if let Some(m) = validated.model {
            entry.insert("model".to_string(), Value::String(m));
        }
        let lvls: Vec<Value> = validated
            .levels
            .iter()
            .map(|l| Value::String(l.as_str().to_string()))
            .collect();
        entry.insert("levels".to_string(), Value::Array(lvls));
        if let Some(n) = validated.notes {
            entry.insert("notes".to_string(), Value::String(n));
        }

        members_obj.insert(member_id, Value::Object(entry));
        Ok(())
    }) {
        return write_error_response(e, DEPARTMENTS_CONFLICT, DEPARTMENTS_HEADING);
    }

    departments_response(st)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberDeleteBody {
    version: String,
    id: String,
}

async fn delete_member(
    State(st): State<Arc<AppState>>,
    Json(body): Json<MemberDeleteBody>,
) -> Response {
    run_write(move || delete_member_blocking(&st, body)).await
}

fn delete_member_blocking(st: &AppState, body: MemberDeleteBody) -> Response {
    let cfg = live_cfg(st);
    let path = departments::departments_path();
    let (text, version) = match read_for_write(&path) {
        Ok(x) => x,
        Err(resp) => return resp,
    };
    let loaded = departments::load_from(text.as_deref().unwrap_or("{}"), &cfg);
    if !loaded.errors.is_empty() {
        return invalid_file(DEPARTMENTS_FILE, &loaded.errors);
    }
    if version != body.version {
        return conflict(DEPARTMENTS_CONFLICT);
    }
    if loaded.get(&body.id).is_none() {
        return refusal(
            DEPARTMENTS_HEADING,
            vec![FieldError {
                field: "id".to_string(),
                message: format!("Không tìm thấy thành viên '{}'", body.id),
            }],
        );
    }

    let member_id = body.id.clone();
    if let Err(e) = settings_store::write_object(&path, &body.version, |root| {
        let mut found = false;
        for (_k, v) in root.iter_mut() {
            if let Some(obj) = v.as_object_mut() {
                if let Some(members) = obj.get_mut("members").and_then(|m| m.as_object_mut()) {
                    if members.remove(&member_id).is_some() {
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            return Err(vec![FieldError {
                field: "id".to_string(),
                message: format!("Không tìm thấy thành viên '{member_id}'"),
            }]);
        }
        Ok(())
    }) {
        return write_error_response(e, DEPARTMENTS_CONFLICT, DEPARTMENTS_HEADING);
    }

    departments_response(st)
}

#[derive(serde::Deserialize)]
struct ModelsQuery {
    engine: Option<String>,
}

async fn get_models(
    State(st): State<Arc<AppState>>,
    Query(query): Query<ModelsQuery>,
) -> Response {
    let cfg = live_cfg(&st);
    let engine_id = query.engine.unwrap_or_default().trim().to_string();
    if engine_id.is_empty() {
        return json_response(StatusCode::OK, json!({ "models": [] }));
    }

    // 1. claude / anthropic native
    if engine_id == "claude" {
        let mut models = vec![
            "claude-opus-5-5".to_string(),
            "claude-sonnet-5-5".to_string(),
            "claude-haiku-5-5".to_string(),
        ];
        for m in cfg.models.values() {
            if !models.contains(m) {
                models.push(m.clone());
            }
        }
        return json_response(StatusCode::OK, json!({ "models": models }));
    }

    // 2. agy engine
    let is_agy = engine_id == "agy"
        || cfg
            .engines
            .get(&engine_id)
            .map(|e| e.kind() == crate::config::EngineKind::Agy)
            .unwrap_or(false);

    if is_agy {
        let agy_cmd = if let Some(def) = cfg.engines.get(&engine_id) {
            if let Some(a) = def.agy() {
                a.command.clone()
            } else {
                "agy".to_string()
            }
        } else if let Some(first_agy) = cfg.engines.list.iter().find_map(|e| e.agy()) {
            first_agy.command.clone()
        } else {
            "agy".to_string()
        };

        // Kiểm tra cache trong bộ nhớ (10 phút)
        if let Ok(guard) = AGY_CACHE.lock() {
            if let Some((instant, cached_cmd, cached_models)) = guard.as_ref() {
                if instant.elapsed() < Duration::from_secs(600) && cached_cmd == &agy_cmd {
                    return json_response(StatusCode::OK, json!({ "models": cached_models }));
                }
            }
        }

        // Chạy `<command> models` với stdin /dev/null, timeout 20s
        let mut cmd = tokio::process::Command::new(&agy_cmd);
        cmd.arg("models");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        match cmd.spawn() {
            Ok(child) => {
                match tokio::time::timeout(Duration::from_secs(20), child.wait_with_output()).await {
                    Ok(Ok(output)) if output.status.success() => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let mut models = Vec::new();
                        for line in stdout.lines() {
                            if line.contains('\t') {
                                if let Some(col0) = line.split('\t').next() {
                                    let trimmed = col0.trim();
                                    if !trimmed.is_empty() {
                                        models.push(trimmed.to_string());
                                    }
                                }
                            }
                        }
                        if let Ok(mut guard) = AGY_CACHE.lock() {
                            *guard = Some((Instant::now(), agy_cmd, models.clone()));
                        }
                        return json_response(StatusCode::OK, json!({ "models": models }));
                    }
                    Ok(Ok(output)) => {
                        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let msg = if err.is_empty() {
                            format!("Lệnh {} models thoát với trạng thái {}", agy_cmd, output.status)
                        } else {
                            err
                        };
                        return json_response(StatusCode::OK, json!({ "models": [], "error": msg }));
                    }
                    Ok(Err(e)) => {
                        return json_response(
                            StatusCode::OK,
                            json!({ "models": [], "error": format!("Lỗi chờ lệnh models: {e}") }),
                        );
                    }
                    Err(_) => {
                        return json_response(
                            StatusCode::OK,
                            json!({ "models": [], "error": "Hết thời gian chờ lệnh models (20s)" }),
                        );
                    }
                }
            }
            Err(e) => {
                return json_response(
                    StatusCode::OK,
                    json!({ "models": [], "error": format!("Không khởi chạy được {agy_cmd}: {e}") }),
                );
            }
        }
    }

    // 3. Khác: models trong config
    let mut models = Vec::new();
    if let Some(def) = cfg.engines.get(&engine_id) {
        match &def.spec {
            crate::config::EngineSpec::Anthropic(a) => {
                for m in a.models.values() {
                    if !models.contains(m) {
                        models.push(m.clone());
                    }
                }
            }
            crate::config::EngineSpec::Agy(a) => {
                if let Some(m) = &a.model {
                    if !models.contains(m) {
                        models.push(m.clone());
                    }
                }
                for m in a.models.values() {
                    if !models.contains(m) {
                        models.push(m.clone());
                    }
                }
            }
            _ => {}
        }
    }
    for m in cfg.models.values() {
        if !models.contains(m) {
            models.push(m.clone());
        }
    }

    json_response(StatusCode::OK, json!({ "models": models }))
}

#[derive(serde::Deserialize)]
struct LevelBody {
    level: String,
}

async fn post_task_level(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<LevelBody>,
) -> Response {
    let level_str = body.level.trim();
    let level = match Level::parse(level_str) {
        Some(l) => l,
        None => return json_error(StatusCode::BAD_REQUEST, "Độ khó không hợp lệ (phải là de, vua, kho)"),
    };

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM tasks WHERE id = ?1",
                rusqlite::params![id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !exists {
            return Ok(Err((StatusCode::NOT_FOUND, format!("Không tìm thấy task #{id}"))));
        }

        let running_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM runs WHERE task_id = ?1 AND status = 'running'",
            rusqlite::params![id],
            |r| r.get(0),
        )?;
        if running_count > 0 {
            return Ok(Err((
                StatusCode::CONFLICT,
                "Không thể đổi độ khó khi task đang có tiến trình chạy".to_string(),
            )));
        }

        crate::db::set_task_level(&conn, id, level.as_str())?;
        Ok(Ok(level.as_str()))
    })
    .await;

    match res {
        Ok(Ok(lvl)) => json_response(StatusCode::OK, json!({ "ok": true, "id": id, "level": lvl })),
        Ok(Err((status, msg))) => json_error(status, &msg),
        Err(e) => internal_save(&format!("Lỗi đổi độ khó task: {e}")),
    }
}

async fn post_proposal_level(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<LevelBody>,
) -> Response {
    let level_str = body.level.trim();
    let level = match Level::parse(level_str) {
        Some(l) => l,
        None => return json_error(StatusCode::BAD_REQUEST, "Độ khó không hợp lệ (phải là de, vua, kho)"),
    };

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let prop = match crate::db::get_proposal(&conn, id)? {
            Some(p) => p,
            None => return Ok(Err((StatusCode::NOT_FOUND, format!("Không tìm thấy đề xuất #{id}")))),
        };

        if prop.status != "pending" && prop.status != "approved" {
            return Ok(Err((
                StatusCode::CONFLICT,
                "Chỉ có thể đổi độ khó khi đề xuất đang ở trạng thái pending hoặc approved".to_string(),
            )));
        }

        crate::db::set_proposal_level(&conn, id, level.as_str())?;
        Ok(Ok(level.as_str()))
    })
    .await;

    match res {
        Ok(Ok(lvl)) => json_response(StatusCode::OK, json!({ "ok": true, "id": id, "level": lvl })),
        Ok(Err((status, msg))) => json_error(status, &msg),
        Err(e) => internal_save(&format!("Lỗi đổi độ khó đề xuất: {e}")),
    }
}

#[derive(serde::Deserialize)]
struct ScoreBody {
    score: i64,
    #[serde(default)]
    note: Option<String>,
}

async fn post_run_score(
    State(st): State<Arc<AppState>>,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<ScoreBody>,
) -> Response {
    if !(1..=5).contains(&body.score) {
        return json_error(StatusCode::BAD_REQUEST, "Điểm đánh giá phải từ 1 đến 5");
    }
    if let Some(note) = &body.note {
        if note.chars().count() > 500 {
            return json_error(StatusCode::BAD_REQUEST, "Ghi chú quá dài (tối đa 500 ký tự)");
        }
    }

    let note = body.note.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let res = blocking(move || {
        let conn = crate::db::open(&st.db_path)?;
        let run_opt: Option<(Option<String>, String, Option<i64>)> = conn
            .query_row(
                "SELECT member_id, status, task_id FROM runs WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        let (member_id_opt, status, task_id) = match run_opt {
            Some(row) => row,
            None => return Ok(Err((StatusCode::NOT_FOUND, format!("Không tìm thấy run #{id}")))),
        };

        let member_id = match member_id_opt {
            Some(m) if !m.is_empty() => m,
            _ => return Ok(Err((StatusCode::BAD_REQUEST, "Run này không có thành viên phòng ban".to_string()))),
        };

        if status == "running" {
            return Ok(Err((StatusCode::BAD_REQUEST, "Không thể chấm điểm khi run đang chạy".to_string())));
        }

        crate::db::record_member_score(&conn, id, &member_id, task_id, body.score as i32, note.as_deref())?;
        let (avg, count) = crate::db::member_score_avg(&conn, &member_id)?;
        Ok(Ok((member_id, avg, count)))
    })
    .await;

    match res {
        Ok(Ok((member_id, avg, count))) => json_response(
            StatusCode::OK,
            json!({
                "ok": true,
                "run_id": id,
                "member_id": member_id,
                "score": body.score,
                "avg_score": avg,
                "score_count": count
            }),
        ),
        Ok(Err((status, msg))) => json_error(status, &msg),
        Err(e) => internal_save(&format!("Lỗi chấm điểm run: {e}")),
    }
}
