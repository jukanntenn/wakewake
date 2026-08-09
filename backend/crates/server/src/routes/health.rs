//! 健康检查路由（api-design.md §1.4 健康检查）。不查 DB。

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health))
}

async fn health(State(_state): State<Arc<AppState>>) -> (StatusCode, Json<serde_json::Value>) {
    // 不查 DB（LB / Docker healthcheck 用）。
    (StatusCode::OK, Json(json!({"status": "ok"})))
}
