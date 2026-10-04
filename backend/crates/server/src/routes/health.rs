//! 健康检查路由（api-design.md §1.4 健康检查）。不查 DB。

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        // 维护状态（ui-ux-risk-control §2.4，前端横幅用，公开端点）。
        .route("/health/maintenance", get(maintenance_status))
        // 部署校验（scripts/check_deploy.py 比对 git_sha 判断"跑的是不是刚部署的镜像"）。
        .route("/version", get(version))
}

async fn health(State(_state): State<Arc<AppState>>) -> (StatusCode, Json<serde_json::Value>) {
    // 不查 DB（LB / Docker healthcheck 用）。
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn version() -> Json<serde_json::Value> {
    // WAKEWAKE_GIT_SHA 由构建注入（docker/build.py --build-arg / CI），
    // 缺省 unknown（本地 cargo run / e2e 构建无 sha）。
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "git_sha": option_env!("WAKEWAKE_GIT_SHA").unwrap_or("unknown"),
    }))
}

async fn maintenance_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let m = state.maintenance.snapshot();
    Json(json!({
        "enabled": m.enabled,
        "mode": m.mode.as_str(),
        "message": m.message,
    }))
}
