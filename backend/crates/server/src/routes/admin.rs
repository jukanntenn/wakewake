//! Admin 路由（api-design.md §admin）。JWT 域 + superuser 守卫。
//!
//! 挂在 /admin/* 下（main.rs `build_router）。顺序：jwt_middleware` → `admin_guard` → handler。
//! 非管理员访问返 `404（防枚举，admin_guard` 实现）。
//!
//! 风控第一版端点：
//! - GET    /admin/stats                          全局聚合计数
//! - GET    /admin/users（分页 + ?is_active&page&page_size）
//! - POST   /admin/users/:id/disable
//! - POST   /admin/users/:id/enable
//! - POST   /admin/users/:id/reset-password       管理员设新密码 + 吊销所有 refresh
//! - POST   /admin/users/:id/verify-email         强制标记邮箱已验证
//! - GET    /admin/agents                         跨用户 agent 列表
//! - GET    /admin/devices                        跨用户 device 列表
//! - GET    /admin/wakes                          跨用户 wake 审计（游标分页）
//! - POST   /admin/devices/:did/resync            强制重同步设备
//! - POST   /admin/integrations/:id/resync        强制重同步集成
//! - POST   /admin/agents/:id/disconnect          强制断开 agent SSE
//! - GET    /admin/audit-log                      审计日志（分页 + ?action）

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::hub::CommandDispatcher;
use crate::middleware::auth::AuthUser;
use crate::service::admin_service;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/stats", get(stats))
        // 用户管理
        .route("/admin/users", get(list_users))
        .route("/admin/users/{id}/disable", post(disable_user))
        .route("/admin/users/{id}/enable", post(enable_user))
        .route("/admin/users/{id}/reset-password", post(reset_password))
        .route("/admin/users/{id}/verify-email", post(verify_email))
        // 跨用户资源列表
        .route("/admin/agents", get(list_agents))
        .route("/admin/devices", get(list_devices))
        .route("/admin/wakes", get(list_wakes))
        // 风控操作
        .route("/admin/devices/{did}/resync", post(resync_device))
        .route("/admin/integrations/{id}/resync", post(resync_integration))
        .route("/admin/agents/{id}/disconnect", post(disconnect_agent))
        // 审计日志
        .route("/admin/audit-log", get(audit_log))
}

// ============================================================================
// 统计
// ============================================================================

#[derive(Debug, Serialize)]
struct StatsResponse {
    users: i64,
    active_users: i64,
    devices: i64,
    agents: i64,
    integrations: i64,
    wakes: i64,
    devices_syncing: i64,
    devices_sync_error: i64,
    /// 当前 hub 在线 agent 数（SSE 连接在，纯内存实时值）。
    online_agents: i64,
}

impl StatsResponse {
    fn from_stats(s: admin_service::GlobalStats, online_agents: i64) -> Self {
        Self {
            users: s.users,
            active_users: s.active_users,
            devices: s.devices,
            agents: s.agents,
            integrations: s.integrations,
            wakes: s.wakes,
            devices_syncing: s.devices_cloud_syncing,
            devices_sync_error: s.devices_cloud_error,
            online_agents,
        }
    }
}

async fn stats(State(state): State<Arc<AppState>>) -> AppResult<Json<StatsResponse>> {
    let s = admin_service::global_stats(&state.pool)
        .await
        .map_err(AppError::from_repo)?;
    let online = i64::try_from(state.hub.connected_count()).unwrap_or(i64::MAX);
    Ok(Json(StatsResponse::from_stats(s, online)))
}

// ============================================================================
// 用户管理
// ============================================================================

#[derive(Debug, Deserialize)]
struct ListUsersQuery {
    is_active: Option<bool>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminUserResponse {
    id: i64,
    email: String,
    is_active: bool,
    disabled_at: Option<String>,
    is_superuser: bool,
    email_verified: bool,
    last_login: Option<String>,
    created_at: String,
}

impl AdminUserResponse {
    fn from_user(u: &crate::domain::user::User) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            is_active: u.is_active,
            disabled_at: u.disabled_at.and_then(|t| t.format(&Rfc3339).ok()),
            is_superuser: u.is_superuser,
            email_verified: u.email_verified,
            last_login: u.last_login.and_then(|t| t.format(&Rfc3339).ok()),
            created_at: u.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ListEnvelope<T> {
    items: Vec<T>,
    page: i64,
    page_size: i64,
    total: i64,
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListUsersQuery>,
) -> AppResult<Json<ListEnvelope<AdminUserResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let (items, total) =
        admin_service::list_users(&state.pool, q.is_active, page, page_size).await?;
    Ok(Json(ListEnvelope {
        items: items.iter().map(AdminUserResponse::from_user).collect(),
        page,
        page_size,
        total,
    }))
}

async fn disable_user(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<AdminUserResponse>)> {
    let user = admin_service::disable_user(&state, actor.user_id, id).await?;
    Ok((StatusCode::OK, Json(AdminUserResponse::from_user(&user))))
}

async fn enable_user(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<AdminUserResponse>)> {
    let user = admin_service::enable_user(&state, actor.user_id, id).await?;
    Ok((StatusCode::OK, Json(AdminUserResponse::from_user(&user))))
}

#[derive(Debug, Deserialize)]
struct ResetPasswordBody {
    new_password: String,
}

async fn reset_password(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
    Json(body): Json<ResetPasswordBody>,
) -> AppResult<StatusCode> {
    admin_service::reset_password(&state, actor.user_id, id, &body.new_password).await?;
    Ok(StatusCode::OK)
}

async fn verify_email(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    admin_service::verify_email(&state, actor.user_id, id).await?;
    Ok(StatusCode::OK)
}

// ============================================================================
// 跨用户 agent 列表
// ============================================================================

#[derive(Debug, Deserialize)]
struct ListAgentsQuery {
    user_id: Option<i64>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminAgentResponse {
    id: i64,
    user_id: i64,
    user_email: String,
    aid: String,
    name: String,
    status: String,
    last_seen: Option<String>,
    created_at: String,
}

impl AdminAgentResponse {
    fn from_row(r: &admin_service::AdminAgentRow, online: bool) -> Self {
        use crate::domain::agent::AgentStatus;
        let status = AgentStatus::classify(r.has_public_key, online);
        Self {
            id: r.id,
            user_id: r.user_id,
            user_email: r.user_email.clone(),
            aid: r.aid.simple().to_string(),
            name: r.name.clone(),
            status: match status {
                AgentStatus::Pending => "pending",
                AgentStatus::Online => "online",
                AgentStatus::Offline => "offline",
            }
            .to_string(),
            last_seen: r.last_seen.and_then(|t| t.format(&Rfc3339).ok()),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

async fn list_agents(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListAgentsQuery>,
) -> AppResult<Json<ListEnvelope<AdminAgentResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let rows = admin_service::list_all_agents(&state.pool, q.user_id, page, page_size)
        .await
        .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_agents(&state.pool, q.user_id)
        .await
        .map_err(AppError::from_repo)?;
    let items = rows
        .iter()
        .map(|r| AdminAgentResponse::from_row(r, state.hub.is_online(r.id)))
        .collect();
    Ok(Json(ListEnvelope {
        items,
        page,
        page_size,
        total,
    }))
}

// ============================================================================
// 跨用户 device 列表
// ============================================================================

#[derive(Debug, Deserialize)]
struct ListDevicesQuery {
    user_id: Option<i64>,
    /// device-sync-v3：过滤维度从 sync_status 改为派生 cloud_status。
    cloud_status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminDeviceResponse {
    did: String,
    user_id: i64,
    user_email: String,
    name: String,
    mac_display: String,
    description: Option<String>,
    /// 派生 cloud_status（§8.3）。
    cloud_status: String,
    last_error: Option<String>,
    last_drift_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl AdminDeviceResponse {
    fn from_row(r: &admin_service::AdminDeviceRow) -> Self {
        Self {
            did: r.did.simple().to_string(),
            user_id: r.user_id,
            user_email: r.user_email.clone(),
            name: r.name.clone(),
            mac_display: r.mac_display.clone(),
            description: r.description.clone(),
            cloud_status: r.cloud_status.clone(),
            last_error: r.last_error.clone(),
            last_drift_at: r
                .last_drift_at
                .as_ref()
                .and_then(|t| t.format(&Rfc3339).ok()),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
            updated_at: r.updated_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

async fn list_devices(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListDevicesQuery>,
) -> AppResult<Json<ListEnvelope<AdminDeviceResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    // 校验 cloud_status 取值（§8.3 五值）。
    if let Some(ref s) = q.cloud_status {
        if !matches!(
            s.as_str(),
            "not_observed" | "syncing" | "synced" | "error" | "no_integration"
        ) {
            return Err(AppError::code(ErrorCode::DeviceNotFound));
        }
    }
    let items = admin_service::list_all_devices(
        &state.pool,
        q.user_id,
        q.cloud_status.as_deref(),
        page,
        page_size,
    )
    .await
    .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_devices(&state.pool, q.user_id, q.cloud_status.as_deref())
        .await
        .map_err(AppError::from_repo)?;
    Ok(Json(ListEnvelope {
        items: items.iter().map(AdminDeviceResponse::from_row).collect(),
        page,
        page_size,
        total,
    }))
}

// ============================================================================
// 跨用户 wake 审计（游标分页）
// ============================================================================

#[derive(Debug, Deserialize)]
struct ListWakesQuery {
    user_id: Option<i64>,
    before: Option<String>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminWakeResponse {
    id: i64,
    user_id: i64,
    user_email: String,
    device_did: String,
    device_name: String,
    r#type: String,
    status: String,
    message: Option<String>,
    created_at: String,
}

impl AdminWakeResponse {
    fn from_row(r: &admin_service::AdminWakeRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            user_email: r.user_email.clone(),
            device_did: r.device_did.simple().to_string(),
            device_name: r.device_name.clone(),
            r#type: r.r#type.clone(),
            status: r.status.clone(),
            message: r.message.clone(),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

async fn list_wakes(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListWakesQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let before = q
        .before
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());

    let items = admin_service::list_all_wakes(&state.pool, q.user_id, before, page_size)
        .await
        .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_wakes(&state.pool, q.user_id, before)
        .await
        .map_err(AppError::from_repo)?;
    let items: Vec<_> = items.iter().map(AdminWakeResponse::from_row).collect();
    Ok(Json(json!({
        "items": items,
        "page_size": page_size,
        "total": total,
    })))
}

// ============================================================================
// 风控操作
// ============================================================================

async fn resync_device(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(did): Path<Uuid>,
) -> AppResult<StatusCode> {
    admin_service::resync_device(&state, actor.user_id, did).await?;
    Ok(StatusCode::OK)
}

async fn resync_integration(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    admin_service::resync_integration(&state, actor.user_id, id).await?;
    Ok(StatusCode::OK)
}

async fn disconnect_agent(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    admin_service::disconnect_agent(&state, actor.user_id, id).await?;
    Ok(StatusCode::OK)
}

// ============================================================================
// 审计日志
// ============================================================================

#[derive(Debug, Deserialize)]
struct AuditLogQuery {
    action: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AuditLogResponse {
    id: i64,
    actor_id: i64,
    actor_email: String,
    action: String,
    target_user_id: Option<i64>,
    target_agent_id: Option<i64>,
    target_device_did: Option<String>,
    detail: serde_json::Value,
    created_at: String,
}

impl AuditLogResponse {
    fn from_row(r: &admin_service::AdminActionRow) -> Self {
        Self {
            id: r.id,
            actor_id: r.actor_id,
            actor_email: r.actor_email.clone(),
            action: r.action.clone(),
            target_user_id: r.target_user_id,
            target_agent_id: r.target_agent_id,
            target_device_did: r.target_device_did.map(|u| u.simple().to_string()),
            detail: r.detail.0.clone(),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

async fn audit_log(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuditLogQuery>,
) -> AppResult<Json<ListEnvelope<AuditLogResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let items = admin_service::list_actions(&state.pool, q.action.as_deref(), page, page_size)
        .await
        .map_err(AppError::from_repo)?;
    let total = admin_service::count_actions(&state.pool, q.action.as_deref())
        .await
        .map_err(AppError::from_repo)?;
    Ok(Json(ListEnvelope {
        items: items.iter().map(AuditLogResponse::from_row).collect(),
        page,
        page_size,
        total,
    }))
}
