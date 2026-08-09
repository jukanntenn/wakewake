//! 设备路由（device-sync-v3 §8.3）。JWT 域。
//!
//! GET/POST /devices；GET/PATCH/DELETE /devices/:did；POST /devices/:did/wake（202 + Location）。
//! 永不返回 `mac_encrypted，只有` `mac_display`。响应字段全派生（cloud_status / projection_status）。

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::device::{CreateDeviceInput, UpdateDeviceInput};
use crate::domain::sync::{CloudStatus, ProjectionStatus, cloud_status, projection_status};
use crate::error::AppResult;
use crate::hub::{self, CommandDispatcher};
use crate::middleware::auth::AuthUser;
use crate::repo::agent_repo;
use crate::service::device_service;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/devices", get(list).post(create))
        .route("/devices/{did}", get(get_one).patch(update).delete(delete))
        .route("/devices/{did}/wake", post(wake))
}

#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub did: String,
    pub name: String,
    pub mac_display: String,
    pub description: Option<String>,
    pub agent_online: bool,
    pub projection_status: String,
    pub cloud_status: String,
    pub cloud_observed_name: Option<String>,
    pub cloud_observed_at: Option<String>,
    pub last_drift_at: Option<String>,
    pub last_drift_kind: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl DeviceResponse {
    /// 派生 DeviceResponse（§8.3）。需 hub（agent_online + acked version）+ agent projection_version
    /// + 是否有 bemfa 集成 + 设备观测三态。
    /// `bemfa_present_enabled`：是否存在启用的 bemfa 集成（device 属同一 agent）。
    #[allow(clippy::too_many_arguments)]
    async fn from_device(
        state: &AppState,
        d: &crate::domain::device::Device,
        bemfa_present_enabled: bool,
    ) -> Self {
        use time::format_description::well_known::Rfc3339;
        // 派生 agent_online + projection_status
        let agent_online = state.hub.is_online(d.agent_id);
        let (proj_status, agent_online) = match agent_repo::find_by_id(&state.pool, d.agent_id)
            .await
            .ok()
            .flatten()
        {
            Some(agent) => {
                let acked = hub::acked_version(&state.hub, d.agent_id).unwrap_or(0);
                let ps = projection_status(agent_online, acked, agent.projection_version);
                (ps.as_str().to_string(), agent_online)
            },
            None => (ProjectionStatus::AgentOffline.as_str().to_string(), false),
        };

        // 派生 cloud_status（§8.3）
        let cs = cloud_status(
            bemfa_present_enabled,
            bemfa_present_enabled,
            d.bemfa_observed_at.is_some(),
            d.bemfa_observed_name.as_deref(),
            &d.name,
            d.last_error.as_deref(),
        );

        Self {
            did: d.did.simple().to_string(),
            name: d.name.clone(),
            mac_display: d.mac_display.clone(),
            description: d.description.clone(),
            agent_online,
            projection_status: proj_status,
            cloud_status: cs.as_str().to_string(),
            cloud_observed_name: d.bemfa_observed_name.clone(),
            cloud_observed_at: d
                .bemfa_observed_at
                .as_ref()
                .and_then(|t| t.format(&Rfc3339).ok()),
            last_drift_at: d
                .last_drift_at
                .as_ref()
                .and_then(|t| t.format(&Rfc3339).ok()),
            last_drift_kind: d.last_drift_kind.clone(),
            last_error: d.last_error.clone(),
            created_at: d.created_at.format(&Rfc3339).unwrap_or_default(),
            updated_at: d.updated_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

/// 查设备的 agent 是否有启用的 bemfa 集成（cloud_status:no_integration 判定用）。
async fn bemfa_present_enabled(state: &AppState, agent_id: i64) -> bool {
    crate::repo::integration_repo::list_by_agent(&state.pool, agent_id)
        .await
        .is_ok_and(|ints| ints.iter().any(|i| i.provider == "bemfa" && i.enabled))
}

async fn list(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let devices = device_service::list(&state.pool, user.user_id).await?;
    let mut items = Vec::with_capacity(devices.len());
    for d in &devices {
        let bpe = bemfa_present_enabled(&state, d.agent_id).await;
        items.push(DeviceResponse::from_device(&state, d, bpe).await);
    }
    let total = items.len() as i64;
    Ok(Json(json!({
        "items": items,
        "page": 1,
        "page_size": total,
        "total": total,
    })))
}

async fn create(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(input): Json<CreateDeviceInput>,
) -> AppResult<(StatusCode, Json<DeviceResponse>)> {
    let device = device_service::create(&state.pool, &state.hub, user.user_id, &input).await?;
    let bpe = bemfa_present_enabled(&state, device.agent_id).await;
    Ok((
        StatusCode::CREATED,
        Json(DeviceResponse::from_device(&state, &device, bpe).await),
    ))
}

async fn get_one(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(did): Path<Uuid>,
) -> AppResult<Json<DeviceResponse>> {
    let device = device_service::get(&state.pool, user.user_id, did).await?;
    let bpe = bemfa_present_enabled(&state, device.agent_id).await;
    Ok(Json(
        DeviceResponse::from_device(&state, &device, bpe).await,
    ))
}

async fn update(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(did): Path<Uuid>,
    Json(input): Json<UpdateDeviceInput>,
) -> AppResult<Json<DeviceResponse>> {
    let device = device_service::update(&state.pool, &state.hub, user.user_id, did, &input).await?;
    let bpe = bemfa_present_enabled(&state, device.agent_id).await;
    Ok(Json(
        DeviceResponse::from_device(&state, &device, bpe).await,
    ))
}

async fn delete(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(did): Path<Uuid>,
) -> AppResult<StatusCode> {
    device_service::delete(&state.pool, &state.hub, user.user_id, did).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 唤醒：202 + Location: /commands/:id（命令已派发但未完成）。
async fn wake(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(did): Path<Uuid>,
) -> AppResult<axum::response::Response> {
    let command_id = device_service::wake(&state.pool, &state.hub, user.user_id, did).await?;
    let body = json!({
        "command_id": command_id,
        "type": "wol",
        "status": "pending",
        "device_id": did.simple().to_string(),
    });
    let response = (
        StatusCode::ACCEPTED,
        [
            ("Location", format!("/api/v1/commands/{command_id}")),
            ("Content-Type", "application/json".to_string()),
        ],
        Json(body),
    )
        .into_response();
    Ok(response)
}

// 保留 CloudStatus import 标记（供未来扩展用，如 admin 视图）
#[allow(dead_code)]
fn _cloud_status_marker() -> CloudStatus {
    CloudStatus::NoIntegration
}
