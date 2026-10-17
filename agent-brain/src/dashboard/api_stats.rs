//! Owning plan: 05-06

use crate::dashboard::AppState;
use axum::Router;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
}
