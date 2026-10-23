//! API bảng công việc: đọc trực tiếp lộ trình GSD của dự án, chỉ đọc, không ghi.
//!
//! Không mở brain.db, không cache giữa các request, không ghép chuỗi từ client vào đường dẫn:
//! thư mục dự án chỉ đến từ projects_map.json qua `Project::root()`.

use crate::dashboard::board::{self, EpicSel, Filter, PhaseNum};
use crate::dashboard::board_scan::{self, Caps, SCAN_BUDGET};
use crate::dashboard::{blocking, json_error, AppState};
use crate::projects::Project;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub const PARAM_MAX_PROJECT: usize = 128;
pub const PARAM_MAX_EPIC: usize = 32;
pub const PARAM_MAX_MILESTONE: usize = 64;
pub const PARAM_MAX_PROFILE: usize = 32;
pub const PROJECTS_LIST_MAX: usize = 500;

#[derive(serde::Deserialize)]
struct BoardQuery {
    project: Option<String>,
    epic: Option<String>,
    milestone: Option<String>,
    profile: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/board", get(get_board))
}

/// Dự án có bảng khi `.planning` là thư mục thật và `ROADMAP.md` là file thường.
pub fn has_board(root: &Path) -> bool {
    let planning = root.join(".planning");
    if !board_scan::is_real_dir(&planning) {
        return false;
    }
    match std::fs::symlink_metadata(planning.join("ROADMAP.md")) {
        Ok(m) => m.file_type().is_file(),
        Err(_) => false,
    }
}

fn json_ok(val: Value) -> Response {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        axum::Json(val),
    )
        .into_response()
}

fn bad_request(message: &str) -> Response {
    json_error(StatusCode::BAD_REQUEST, message)
}

fn empty_state(
    reason: &str,
    message: &str,
    expected_path: Option<String>,
    project: Option<Value>,
    projects: Vec<Value>,
) -> Response {
    json_ok(json!({
        "state": "empty",
        "reason": reason,
        "message": message,
        "expected_path": expected_path,
        "project": project,
        "projects": projects,
    }))
}

fn project_summary(p: &Project) -> Value {
    json!({
        "id": p.id,
        "name_vi": p.name_vi,
        "profile": p.profile,
        "type": p.kind,
    })
}

/// Dự án media, thiếu thư mục hoặc không có `.planning/ROADMAP.md` thật thì không có bảng.
fn board_flag(p: &Project) -> bool {
    p.kind != "media" && !p.missing && has_board(&p.root())
}

fn projects_json(entries: &[(&Project, bool)]) -> Vec<Value> {
    entries
        .iter()
        .map(|(p, has)| {
            json!({
                "id": p.id,
                "name_vi": p.name_vi,
                "profile": p.profile,
                "has_board": has,
            })
        })
        .collect()
}

/// Kiểm tra độ dài và ký tự điều khiển của một tham số; `Some` là phản hồi 400.
fn check_param(name: &str, value: Option<&str>, max: usize) -> Option<Response> {
    let v = value?;
    if v.chars().count() > max {
        return Some(bad_request(&format!(
            "Tham số {name} quá dài (tối đa {max} ký tự)"
        )));
    }
    if v.chars().any(|c| c.is_control()) {
        return Some(bad_request(&format!(
            "Tham số {name} chứa ký tự không hợp lệ"
        )));
    }
    None
}

async fn get_board(State(st): State<Arc<AppState>>, Query(query): Query<BoardQuery>) -> Response {
    let res = blocking(move || {
        // 1. Kiểm tra tham số trước mọi thao tác với file.
        let checks = [
            ("project", query.project.as_deref(), PARAM_MAX_PROJECT),
            ("epic", query.epic.as_deref(), PARAM_MAX_EPIC),
            ("milestone", query.milestone.as_deref(), PARAM_MAX_MILESTONE),
            ("profile", query.profile.as_deref(), PARAM_MAX_PROFILE),
        ];
        for (name, value, max) in checks {
            if let Some(resp) = check_param(name, value, max) {
                return Ok(resp);
            }
        }
        let epic = match query.epic.as_deref().map(str::trim) {
            None | Some("") => EpicSel::Default,
            Some("all") => EpicSel::All,
            Some(s) => match PhaseNum::parse(s) {
                Some(n) => EpicSel::Phase(n),
                None => {
                    return Ok(bad_request(
                        "Tham số epic không hợp lệ: dùng all hoặc số phase như 05.4",
                    ))
                }
            },
        };
        let milestone = query
            .milestone
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let profile_opt = query.profile.as_deref().filter(|s| !s.is_empty());
        if let Some(p) = profile_opt {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return Ok(bad_request(&e.message));
            }
        }

        // 2. Danh sách dự án (lọc theo profile) và dự án được chọn.
        let map = crate::projects::load_map(&st.map_path)?;
        let all_projects = crate::projects::projects_of(&map);
        let listed: Vec<&Project> = all_projects
            .iter()
            .filter(|p| profile_opt.is_none_or(|prof| p.profile == prof))
            .take(PROJECTS_LIST_MAX)
            .collect();
        let entries: Vec<(&Project, bool)> = listed.iter().map(|p| (*p, board_flag(p))).collect();
        let list = projects_json(&entries);

        let explicit = query
            .project
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let chosen: &Project = match explicit {
            Some(id) => match all_projects.iter().find(|p| p.id == id) {
                Some(p) => p,
                None => {
                    return Ok(json_error(
                        StatusCode::NOT_FOUND,
                        &format!("Không tìm thấy dự án {id}"),
                    ))
                }
            },
            None => match entries.iter().find(|(_, has)| *has) {
                Some((p, _)) => p,
                None => {
                    return Ok(empty_state(
                        "no_board_projects",
                        "Chưa có dự án nào có bảng công việc. Bảng đọc lộ trình GSD; cần file .planning/ROADMAP.md trong thư mục dự án.",
                        None,
                        None,
                        list,
                    ))
                }
            },
        };

        // 3. Các trạng thái trống, theo đúng thứ tự D-02.
        let summary = project_summary(chosen);
        if chosen.kind == "media" {
            return Ok(empty_state(
                "media",
                "Dự án media không có bảng công việc (bảng đọc lộ trình GSD của dự án code).",
                None,
                Some(summary),
                list,
            ));
        }
        let root = chosen.root();
        let planning = root.join(".planning");
        let expected_str = planning.join("ROADMAP.md").display().to_string();
        let root_is_dir = std::fs::metadata(&root).map(|m| m.is_dir()).unwrap_or(false);
        if chosen.missing || !root_is_dir {
            return Ok(empty_state(
                "missing_folder",
                &format!("Thư mục dự án không còn tồn tại: {}", root.display()),
                Some(expected_str),
                Some(summary),
                list,
            ));
        }
        let no_roadmap = |expected: String, summary: Value, list: Vec<Value>| {
            empty_state(
                "no_roadmap",
                &format!(
                    "Dự án này chưa có bảng công việc. Bảng đọc lộ trình GSD của dự án; cần file {expected}."
                ),
                Some(expected),
                Some(summary),
                list,
            )
        };
        match std::fs::symlink_metadata(&planning) {
            Err(_) => return Ok(no_roadmap(expected_str, summary, list)),
            Ok(m) if m.file_type().is_symlink() => {
                return Ok(empty_state(
                    "planning_symlink",
                    &format!(
                        "Bảng không đọc .planning là symlink — chỉ đọc thư mục thật trong dự án. Cần thư mục {}/.planning chứa ROADMAP.md.",
                        root.display()
                    ),
                    Some(expected_str),
                    Some(summary),
                    list,
                ))
            }
            Ok(m) if !m.file_type().is_dir() => {
                return Ok(no_roadmap(expected_str, summary, list))
            }
            Ok(_) => {}
        }
        if let Err(e) = std::fs::symlink_metadata(planning.join("ROADMAP.md")) {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(no_roadmap(expected_str, summary, list));
            }
        }

        // 4. Đọc file GSD ngay trong request này (D-06: không cache).
        let facts = match board_scan::scan(&root, &Caps::default(), Instant::now() + SCAN_BUDGET) {
            Ok(f) => f,
            Err(refusal) => {
                return Ok(empty_state(
                    refusal.reason,
                    &format!("Không đọc được {expected_str}: {}", refusal.detail),
                    Some(expected_str),
                    Some(summary),
                    list,
                ));
            }
        };
        let view = board::derive(&facts, &Filter { epic, milestone });

        let mut body = serde_json::Map::new();
        body.insert("state".into(), json!("ok"));
        body.insert("reason".into(), Value::Null);
        body.insert("message".into(), Value::Null);
        body.insert("expected_path".into(), json!(expected_str));
        body.insert("project".into(), summary);
        body.insert("projects".into(), Value::Array(list));
        if let Ok(Value::Object(fields)) = serde_json::to_value(&view) {
            for (k, v) in fields {
                body.insert(k, v);
            }
        }
        Ok(json_ok(Value::Object(body)))
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => {
            crate::term::emit(&format!("Lỗi nội bộ khi đọc bảng công việc: {e}"));
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Lỗi nội bộ khi đọc bảng công việc",
            )
        }
    }
}
