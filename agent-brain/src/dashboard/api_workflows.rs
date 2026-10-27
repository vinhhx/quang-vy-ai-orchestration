//! API đọc danh sách quy trình (workflows) cho dashboard.

use crate::dashboard::{blocking, json_error, settings_store, AppState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/workflows", get(get_workflows))
}

/// The `GET /api/workflows` body; also the fresh list a workflow write answers with.
pub fn workflows_payload(st: &AppState) -> crate::error::BrainResult<serde_json::Value> {
    let map = crate::projects::load_map(&st.map_path)?;
    let (loaded, version) = match settings_store::read_doc(&crate::workflows::workflows_path()) {
        Ok((text, version)) => (
            crate::workflows::load_from(text.as_deref(), &st.cfg),
            version,
        ),
        Err(_) => (crate::workflows::load(&st.cfg), "unreadable".to_string()),
    };
    Ok(workflows_json(&loaded, &map, &version))
}

async fn get_workflows(State(st): State<Arc<AppState>>) -> Response {
    let res = blocking(move || {
        let val = workflows_payload(&st)?;
        Ok((
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            )],
            axum::Json(val),
        )
            .into_response())
    })
    .await;

    match res {
        Ok(resp) => resp,
        Err(e) => {
            crate::term::emit(&format!("Lỗi nội bộ khi đọc danh sách quy trình: {e}"));
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Lỗi nội bộ khi đọc danh sách quy trình",
            )
        }
    }
}

pub fn workflows_json(
    loaded: &crate::workflows::LoadedWorkflows,
    map: &serde_json::Value,
    version: &str,
) -> serde_json::Value {
    let projects = crate::projects::projects_of(map);

    let mut project_defaults = serde_json::Map::new();
    for p in &projects {
        let raw_wf = crate::projects::workflow_of(map, &p.id);
        let resolved = crate::workflows::resolve(loaded, None, raw_wf.as_deref());
        let (effective, note) = match resolved {
            Ok(r) => (r.workflow.name, r.note),
            Err(_) => (crate::workflows::DEFAULT_WORKFLOW.to_string(), None),
        };
        project_defaults.insert(
            p.id.clone(),
            serde_json::json!({
                "workflow": raw_wf,
                "effective": effective,
                "note": note,
            }),
        );
    }

    let workflows_list: Vec<serde_json::Value> = loaded
        .workflows
        .iter()
        .map(|wf| {
            let mut default_for = Vec::new();
            for p in &projects {
                let raw_wf = crate::projects::workflow_of(map, &p.id);
                let effective = match crate::workflows::resolve(loaded, None, raw_wf.as_deref()) {
                    Ok(r) => r.workflow.name,
                    Err(_) => crate::workflows::DEFAULT_WORKFLOW.to_string(),
                };
                if effective == wf.name {
                    let name = if !p.name_vi.trim().is_empty() {
                        p.name_vi.clone()
                    } else {
                        p.id.clone()
                    };
                    default_for.push(name);
                }
            }

            serde_json::json!({
                "name": wf.name,
                "source": wf.source,
                "summary": wf.summary(),
                "auto": wf.auto,
                "loops": !wf.loops.is_empty(),
                "loop_names": wf.loops,
                "stages": wf.stages,
                "max_rounds": wf.max_rounds,
                "budget_minutes": wf.budget_minutes,
                "budget_usd": wf.budget_usd,
                "limits": wf.limits_text(),
                "default_for": default_for,
                "edit": wf.to_file_entry(),
            })
        })
        .collect();

    serde_json::json!({
        "workflows": workflows_list,
        "errors": loaded.errors,
        "project_defaults": serde_json::Value::Object(project_defaults),
        "version": version,
    })
}
