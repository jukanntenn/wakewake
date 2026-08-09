//! Agent → Server 上行路由（device-sync-v3 §8.6）。Agent M2M 域（pairing-code）。
//!
//! - `POST /agents/self/commands/{command_id}/complete`（wake 命令终态回报，200）
//! - `POST /agents/self/sync`（观测/ack 同步，200 + current_version）
//! - `POST /agents/self/wakes`（MQTT 触发的语音唤醒上报，201 + id）

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use wakewake_protocol::{CommandResult, SyncRequest, WakeRequest};

use crate::domain::command::CommandOutcome;
use crate::error::AppResult;
use crate::middleware::agent_auth::AuthAgent;
use crate::service::command_handler;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/agents/self/commands/{command_id}/complete",
            post(complete),
        )
        .route("/agents/self/sync", post(sync))
        .route("/agents/self/wakes", post(wakes))
}

async fn complete(
    State(state): State<Arc<AppState>>,
    Path(command_id): Path<String>,
    Json(body): Json<CommandResult>,
) -> AppResult<StatusCode> {
    tracing::info!(
        command_id = %command_id,
        success = body.success,
        message = %body.message,
        "agent command complete received"
    );
    crate::observability::metrics::record_command_completed(body.success);
    let outcome = CommandOutcome {
        success: body.success,
        message: body.message,
    };
    let hit = command_handler::handle_complete(
        &state.pool,
        &state.hub,
        Some(&state.wake_writer),
        &command_id,
        outcome,
    )
    .await?;
    if !hit {
        tracing::warn!(command_id = %command_id, "complete for unknown/stale command ignored");
    }
    Ok(StatusCode::OK)
}

async fn sync(
    State(state): State<Arc<AppState>>,
    agent: AuthAgent,
    Json(req): Json<SyncRequest>,
) -> AppResult<(StatusCode, Json<wakewake_protocol::SyncResponse>)> {
    tracing::info!(
        agent_id = agent.agent_id,
        applied_version = ?req.applied_version,
        has_integration = req.integration.is_some(),
        "agent sync received"
    );
    let resp = command_handler::handle_sync(&state.pool, &state.hub, agent.agent_id, req).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn wakes(
    State(state): State<Arc<AppState>>,
    agent: AuthAgent,
    Json(req): Json<WakeRequest>,
) -> AppResult<(StatusCode, Json<wakewake_protocol::WakeResponse>)> {
    tracing::info!(
        agent_id = agent.agent_id,
        did = %req.did,
        success = req.success,
        "agent wake report (MQTT triggered) received"
    );
    let resp = command_handler::handle_wake_report(&state.pool, agent.agent_id, req).await?;
    Ok((StatusCode::CREATED, Json(resp)))
}
