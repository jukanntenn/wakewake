//! Agent 管理路由（api-design.md §1.4 Agent）。JWT 域，用户管理视角。
//!
//! GET /`agents/default（pairing_code` 按状态脱敏）；POST /agents/default/pairing-code/rotate。
//! 1:1 模型，不暴露 CRUD。

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::domain::agent::AgentStatus;
use crate::error::AppResult;
use crate::hub::CommandDispatcher;
use crate::middleware::auth::AuthUser;
use crate::service::agent_service;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agents/default", get(get_default))
        .route("/agents/default/pairing-code/rotate", post(rotate))
}

#[derive(Debug, Serialize)]
struct DefaultAgentResponse {
    aid: String,
    name: String,
    status: String,
    pairing_code: String,
    public_key: Option<String>,
    last_seen: Option<String>,
    created_at: String,
}

async fn get_default(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> AppResult<Json<DefaultAgentResponse>> {
    use time::format_description::well_known::Rfc3339;
    let agent = agent_service::get_default(&state.pool, user.user_id).await?;
    let status = agent_service::derive_status(&agent, state.hub.is_online(agent.id));
    let pairing_code = agent_service::mask_pairing_code(&agent.pairing_code, status);
    Ok(Json(DefaultAgentResponse {
        aid: agent.aid.simple().to_string(),
        name: agent.name,
        status: match status {
            AgentStatus::Pending => "pending",
            AgentStatus::Online => "online",
            AgentStatus::Offline => "offline",
        }
        .to_string(),
        pairing_code,
        public_key: agent.public_key,
        last_seen: agent.last_seen.and_then(|t| t.format(&Rfc3339).ok()),
        created_at: agent.created_at.format(&Rfc3339).unwrap_or_default(),
    }))
}

async fn rotate(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let agent = agent_service::rotate_pairing_code(&state.pool, user.user_id).await?;
    // rotate 返回完整新码：旧码立即失效，用户必须立即拿到新码才能重新连接 agent。
    // （get_default 仍按状态脱敏，仅在 rotate 这一刻返回完整值。）
    Ok((
        StatusCode::OK,
        Json(json!({
            "pairing_code": agent.pairing_code,
        })),
    ))
}
