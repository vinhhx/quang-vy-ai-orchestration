//! Owning plan: 05-06

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct StatsQuery {
    profile: Option<String>,
    since: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/stats", get(get_stats))
}

async fn get_stats(State(st): State<Arc<AppState>>, Query(query): Query<StatsQuery>) -> Response {
    let res = blocking(move || {
        let profile_opt = query.profile.as_deref().filter(|s| !s.is_empty());
        if let Some(p) = profile_opt {
            if let Err(e) = crate::profile::check_profile(p, &st.cfg.profiles) {
                return Ok(json_error(StatusCode::BAD_REQUEST, &e.message));
            }
        }

        let since_opt = query.since.as_deref().filter(|s| !s.is_empty());
        let since_ms = match since_opt {
            Some(s) => match crate::stats::parse_since(s, crate::db::now_ms()) {
                Ok(ms) => Some(ms),
                Err(e) => return Ok(json_error(StatusCode::BAD_REQUEST, &e.message)),
            },
            None => None,
        };

        let conn = crate::db::open(&st.db_path)?;
        let report = crate::stats::query_report(&conn, profile_opt, since_opt, since_ms)?;
        let by_day = crate::stats::query_by_day(&conn, profile_opt, since_ms)?;
        let by_profile = crate::stats::query_by_profile(&conn, profile_opt, since_ms)?;
        let engines = crate::stats::query_by_engine(&conn, profile_opt, since_ms, &st.cfg.pricing)?;
        let qa_by_dev_engine = crate::stats::query_qa_by_dev_engine(&conn, profile_opt, since_ms)?;

        let mut val = serde_json::to_value(&report)
            .map_err(|e| crate::error::BrainError::error(format!("Lỗi serialize JSON: {e}")))?;
        if let Some(obj) = val.as_object_mut() {
            obj.insert(
                "by_day".to_string(),
                serde_json::to_value(by_day).map_err(|e| {
                    crate::error::BrainError::error(format!("Lỗi serialize by_day: {e}"))
                })?,
            );
            obj.insert(
                "by_profile".to_string(),
                serde_json::to_value(by_profile).map_err(|e| {
                    crate::error::BrainError::error(format!("Lỗi serialize by_profile: {e}"))
                })?,
            );
            obj.insert(
                "engines".to_string(),
                serde_json::to_value(engines).map_err(|e| {
                    crate::error::BrainError::error(format!("Lỗi serialize engines: {e}"))
                })?,
            );
            obj.insert(
                "qa_by_dev_engine".to_string(),
                serde_json::to_value(qa_by_dev_engine).map_err(|e| {
                    crate::error::BrainError::error(format!("Lỗi serialize qa_by_dev_engine: {e}"))
                })?,
            );
        }

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
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    }
}
