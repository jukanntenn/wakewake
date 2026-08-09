//! 命令路由（api-design.md §1.4 命令）。JWT 域。
//!
//! GET /commands/:id（内存资源，60s 过期后 404）。

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::hub::CommandDispatcher;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/commands/{command_id}", get(get_command))
}

#[derive(Debug, Serialize)]
struct CommandResponse {
    command_id: String,
    #[serde(rename = "type")]
    command_type: String,
    status: String,
    result: Option<serde_json::Value>,
    created_at: String,
    completed_at: Option<String>,
}

async fn get_command(
    State(state): State<Arc<AppState>>,
    Path(command_id): Path<String>,
) -> AppResult<Json<CommandResponse>> {
    use time::format_description::well_known::Rfc3339;
    // 命令存在性校验 + id 前缀校验
    crate::service::command_handler::validate_command_id(&command_id)?;

    let command = state
        .hub
        .get_command(&command_id)
        .ok_or(AppError::code(ErrorCode::DeviceNotFound))?; // 404（命令过期/不存在）

    Ok(Json(CommandResponse {
        command_id: command.id,
        command_type: command.payload.type_name().to_string(),
        status: command.status.as_str().to_string(),
        result: command
            .result
            .as_ref()
            .map(|r| json!({"success": r.success, "message": r.message})),
        created_at: command.created_at.format(&Rfc3339).unwrap_or_default(),
        completed_at: command.completed_at.and_then(|t| t.format(&Rfc3339).ok()),
    }))
}
