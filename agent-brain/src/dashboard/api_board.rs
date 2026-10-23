//! API bảng công việc: đọc trực tiếp lộ trình GSD của dự án, chỉ đọc, không ghi.

use crate::dashboard::board::{self, EpicSel, Filter, PhaseNum};
use crate::dashboard::board_scan::{self, Caps, SCAN_BUDGET};
use crate::dashboard::{blocking, json_error, AppState};
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
    #[allow(dead_code)]
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

fn project_summary(p: &crate::projects::Project) -> Value {
    json!({
        "id": p.id,
        "name_vi": p.name_vi,
        "profile": p.profile,
        "type": p.kind,
    })
}

fn projects_list(projects: &[crate::projects::Project]) -> Vec<Value> {
    projects
        .iter()
        .take(PROJECTS_LIST_MAX)
        .map(|p| {
            let has = p.kind != "media" && !p.missing && has_board(&p.root());
            json!({
                "id": p.id,
                "name_vi": p.name_vi,
                "profile": p.profile,
                "has_board": has,
            })
        })
        .collect()
}

async fn get_board(State(st): State<Arc<AppState>>, Query(query): Query<BoardQuery>) -> Response {
    let res = blocking(move || {
        let map = crate::projects::load_map(&st.map_path)?;
        let projects = crate::projects::projects_of(&map);
        let list = projects_list(&projects);

        let wanted = query.project.as_deref().unwrap_or("");
        let Some(project) = projects.iter().find(|p| p.id == wanted) else {
            return Ok(json_error(
                StatusCode::NOT_FOUND,
                &format!("Không tìm thấy dự án {wanted}"),
            ));
        };
        let root = project.root();
        let expected = root.join(".planning").join("ROADMAP.md");
        let expected_str = expected.display().to_string();
        let summary = project_summary(project);

        if !board_scan::is_real_dir(&root.join(".planning")) || !has_board(&root) {
            return Ok(empty_state(
                "no_roadmap",
                &format!(
                    "Dự án này chưa có bảng công việc. Bảng đọc lộ trình GSD của dự án; cần file {expected_str}."
                ),
                Some(expected_str),
                Some(summary),
                list,
            ));
        }

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

        let epic = match query.epic.as_deref().map(str::trim) {
            Some("all") => EpicSel::All,
            Some(s) => match PhaseNum::parse(s) {
                Some(n) => EpicSel::Phase(n),
                None => EpicSel::Default,
            },
            None => EpicSel::Default,
        };
        let milestone = query
            .milestone
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
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
