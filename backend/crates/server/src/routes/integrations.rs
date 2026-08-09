//! 集成路由（device-sync-v3 §8.5）。JWT 域。
//!
//! GET/POST /integrations；GET /integrations/:provider/schema；GET/PATCH/DELETE /integrations/:provider；
//! POST /integrations/:provider/{disable,enable}。响应 status 7 值派生。

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::domain::integration::{CreateIntegrationInput, UpdateIntegrationInput};
use crate::domain::sync::integration_status;
use crate::error::AppResult;
use crate::hub::CommandDispatcher;
use crate::middleware::auth::AuthUser;
use crate::repo::agent_repo;
use crate::service::integration_service;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/integrations", get(list).post(create))
        .route("/integrations/{provider}/schema", get(schema))
        .route(
            "/integrations/{provider}",
            get(get_one).patch(update).delete(delete),
        )
        .route("/integrations/{provider}/disable", post(disable))
        .route("/integrations/{provider}/enable", post(enable))
}

#[derive(Debug, Serialize)]
struct IntegrationResponse {
    provider: String,
    config: serde_json::Value,
    enabled: bool,
    status: String,
    mqtt_connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_report_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl IntegrationResponse {
    /// 派生 IntegrationResponse（§8.5 status 7 值）。
    /// `integration_present=false`（集成不存在）→ status=not_connected。
    async fn from(state: &AppState, i: Option<&crate::domain::integration::Integration>) -> Self {
        use time::format_description::well_known::Rfc3339;
        match i {
            None => Self {
                provider: "bemfa".into(),
                config: serde_json::json!({}),
                enabled: false,
                status: "not_connected".into(),
                mqtt_connected: false,
                last_error: None,
                last_report_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            Some(i) => {
                let agent_online = state.hub.is_online(i.agent_id);
                let status = integration_status(
                    true,
                    agent_online,
                    i.enabled,
                    i.last_error.as_deref(),
                    i.last_report_at.is_some(),
                    i.mqtt_connected,
                );
                Self {
                    provider: i.provider.clone(),
                    config: i.config.clone(),
                    enabled: i.enabled,
                    status: status.as_str().into(),
                    mqtt_connected: i.mqtt_connected,
                    last_error: i.last_error.clone(),
                    last_report_at: i
                        .last_report_at
                        .as_ref()
                        .and_then(|t| t.format(&Rfc3339).ok()),
                    created_at: i.created_at.format(&Rfc3339).unwrap_or_default(),
                    updated_at: i.updated_at.format(&Rfc3339).unwrap_or_default(),
                }
            },
        }
    }
}

async fn list(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let items = integration_service::list(&state.pool, &state.providers, user.user_id).await?;
    let mut responses = Vec::with_capacity(items.len());
    for i in &items {
        responses.push(IntegrationResponse::from(&state, Some(i)).await);
    }
    // §14.6 无集成场景：list 空时前端引导"连接巴法云"。这里如实返回空 items。
    Ok(Json(json!({ "items": responses })))
}

async fn schema(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let s = integration_service::schema(&state.providers, &provider)?;
    Ok(Json(s.clone()))
}

async fn create(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(input): Json<CreateIntegrationInput>,
) -> AppResult<(StatusCode, Json<IntegrationResponse>)> {
    let i = integration_service::create(
        &state.pool,
        &state.providers,
        &state.hub,
        user.user_id,
        &input,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(IntegrationResponse::from(&state, Some(&i)).await),
    ))
}

async fn get_one(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(provider): Path<String>,
) -> AppResult<Json<IntegrationResponse>> {
    // §8.5：集成不存在时 status=not_connected（不返 404，让前端展示"连接巴法云"引导）。
    match integration_service::get(&state.pool, &state.providers, user.user_id, &provider).await {
        Ok(i) => Ok(Json(IntegrationResponse::from(&state, Some(&i)).await)),
        Err(crate::error::AppError::Code(crate::error::ErrorCode::IntegrationNotFound)) => {
            Ok(Json(IntegrationResponse::from(&state, None).await))
        },
        Err(e) => Err(e),
    }
}

async fn update(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(provider): Path<String>,
    Json(input): Json<UpdateIntegrationInput>,
) -> AppResult<Json<IntegrationResponse>> {
    let i = integration_service::update(
        &state.pool,
        &state.providers,
        &state.hub,
        user.user_id,
        &provider,
        &input,
    )
    .await?;
    Ok(Json(IntegrationResponse::from(&state, Some(&i)).await))
}

async fn delete(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    integration_service::delete(&state.pool, &state.hub, user.user_id, &provider).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn disable(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    integration_service::set_enabled(&state.pool, &state.hub, user.user_id, &provider, false)
        .await?;
    Ok(StatusCode::OK)
}

async fn enable(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(provider): Path<String>,
) -> AppResult<StatusCode> {
    integration_service::set_enabled(&state.pool, &state.hub, user.user_id, &provider, true)
        .await?;
    Ok(StatusCode::OK)
}

// 抑制未使用警告（agent_repo 在 admin 视图用，此文件保留 import 以备扩展）
#[allow(dead_code)]
fn _agent_repo_marker() {
    let _ = agent_repo::find_agent_id_by_user;
}
