//! Owning plan: 06.2-02
//!
//! `GET /api/telegram`: read-only status of the Telegram bot (state, bot name, paired chat,
//! outbox size, today's Telegram-origin task count). Behind the usual token auth. The
//! response is built from `telegram::status` plus two counters; it never carries the bot
//! token, any other field of `telegram.json`, or a URL.

use crate::dashboard::{blocking, json_error, AppState};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/telegram", get(get_telegram))
}

async fn get_telegram(State(st): State<Arc<AppState>>) -> Response {
    let res = blocking(move || {
        let snap = crate::telegram::status::snapshot();
        let mut val = serde_json::to_value(&snap).map_err(|e| {
            crate::error::BrainError::error(format!("Lỗi serialize trạng thái Telegram: {e}"))
        })?;

        // The same count the daily cap uses (OD-2): Telegram-origin rows since local midnight.
        let since = crate::telegram::intake::local_midnight_ms(chrono::Local::now());
        let tasks_today = crate::db::open(&st.db_path)
            .and_then(|conn| crate::db::count_tasks_since(&conn, crate::db::ORIGIN_TELEGRAM, since))
            .ok();

        if let Some(obj) = val.as_object_mut() {
            obj.insert("tasks_today".to_string(), serde_json::json!(tasks_today));
            obj.insert(
                "tasks_per_day_limit".to_string(),
                serde_json::json!(st.cfg.telegram_max_tasks_per_day),
            );
            obj.insert(
                "config_path".to_string(),
                serde_json::json!(crate::telegram::config::path().display().to_string()),
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
