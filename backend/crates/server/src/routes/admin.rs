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
        .route("/admin/integrations", get(list_integrations))
        // Activity 统一时间线（login_events UNION admin_actions，§0.3）
        .route("/admin/activity", get(list_activity))
        // 维护模式（§9.10）
        .route("/admin/maintenance", get(get_maintenance).post(set_maintenance))
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
    /// email 前缀搜索（§11.C.4，UX-62）。
    q: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminUserResponse {
    id: i64,
    email: String,
    is_active: bool,
    disabled_at: Option<String>,
    disabled_reason: Option<String>,
    disabled_by: Option<i64>,
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
            disabled_reason: u.disabled_reason.clone(),
            disabled_by: u.disabled_by,
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
        admin_service::list_users(&state.pool, q.is_active, q.q.as_deref(), page, page_size)
            .await?;
    Ok(Json(ListEnvelope {
        items: items.iter().map(AdminUserResponse::from_user).collect(),
        page,
        page_size,
        total,
    }))
}

#[derive(Debug, Default, Deserialize)]
struct DisableUserBody {
    /// 封禁原因（可选，ui-ux-risk-control §5.4/§8.2）。
    #[serde(default)]
    reason: Option<String>,
}

async fn disable_user(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Path(id): Path<i64>,
    body: Option<Json<DisableUserBody>>,
) -> AppResult<(StatusCode, Json<AdminUserResponse>)> {
    let reason = body.and_then(|Json(b)| b.reason);
    let user = admin_service::disable_user(&state, actor.user_id, id, reason.as_deref()).await?;
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
    /// email 前缀搜索（§11.C.4）。
    q: Option<String>,
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
    let rows =
        admin_service::list_all_agents(&state.pool, q.user_id, q.q.as_deref(), page, page_size)
            .await
            .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_agents(&state.pool, q.user_id, q.q.as_deref())
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
    /// device-sync-v3：过滤维度从旧 sync_status 改为派生 cloud_status。
    cloud_status: Option<String>,
    /// device name 或 user email 前缀搜索（§9.6/§11.C.4）。
    q: Option<String>,
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
        q.q.as_deref(),
        page,
        page_size,
    )
    .await
    .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_devices(
        &state.pool,
        q.user_id,
        q.cloud_status.as_deref(),
        q.q.as_deref(),
    )
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
    /// email / device name 前缀搜索（§9.9）。
    q: Option<String>,
    /// wake type 过滤：wol / bemfa_wake（§9.9）。
    wake_type: Option<String>,
    /// result 过滤：success / failed / expired（§9.9）。
    result: Option<String>,
    /// ISO8601 时间范围起点（§9.9 Date 过滤）。
    since: Option<String>,
    /// ISO8601 时间范围终点。
    until: Option<String>,
    page: Option<i64>,
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
) -> AppResult<Json<ListEnvelope<AdminWakeResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let since = q
        .since
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());
    let until = q
        .until
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());

    let items = admin_service::list_all_wakes_offset(
        &state.pool,
        q.user_id,
        q.q.as_deref(),
        q.wake_type.as_deref(),
        q.result.as_deref(),
        since,
        until,
        page,
        page_size,
    )
    .await
    .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_wakes_offset(
        &state.pool,
        q.user_id,
        q.q.as_deref(),
        q.wake_type.as_deref(),
        q.result.as_deref(),
        since,
        until,
    )
    .await
    .map_err(AppError::from_repo)?;
    let items: Vec<_> = items.iter().map(AdminWakeResponse::from_row).collect();
    Ok(Json(ListEnvelope {
        items,
        page,
        page_size,
        total,
    }))
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

// ============================================================================
// Activity 统一时间线（login_events UNION admin_actions，ui-ux-risk-control §0.3/§9.5）
// ============================================================================

#[derive(Debug, Deserialize)]
struct ActivityQuery {
    /// login / audit（可选过滤）。
    kind: Option<String>,
    /// success / failed（仅 login 有效）。
    result: Option<String>,
    /// email / IP / action 前缀搜索。
    q: Option<String>,
    /// ISO8601 时间范围。
    since: Option<String>,
    until: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ActivityResponse {
    kind: String,
    created_at: String,
    actor_id: Option<i64>,
    actor_label: String,
    action: String,
    detail: ActivityDetail,
}

#[derive(Debug, Serialize)]
struct ActivityDetail {
    ip: Option<String>,
    user_agent: Option<String>,
    failure_code: Option<String>,
    target: Option<String>,
    reason: Option<String>,
}

impl ActivityResponse {
    fn from_row(r: &admin_service::ActivityRow) -> Self {
        Self {
            kind: r.kind.clone(),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
            actor_id: r.actor_id,
            actor_label: r.actor_label.clone(),
            action: r.action.clone(),
            detail: ActivityDetail {
                ip: r.detail_ip.clone(),
                user_agent: r.detail_ua.clone(),
                failure_code: r.detail_failure_code.clone(),
                target: r.detail_target.clone(),
                reason: r.detail_reason.clone(),
            },
        }
    }
}

async fn list_activity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ActivityQuery>,
) -> AppResult<Json<ListEnvelope<ActivityResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let since = q
        .since
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());
    let until = q
        .until
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok());
    let items = admin_service::list_activity(
        &state.pool,
        q.kind.as_deref(),
        q.result.as_deref(),
        q.q.as_deref(),
        since,
        until,
        page,
        page_size,
    )
    .await
    .map_err(AppError::from_repo)?;
    let total = admin_service::count_activity(
        &state.pool,
        q.kind.as_deref(),
        q.result.as_deref(),
        q.q.as_deref(),
        since,
        until,
    )
    .await
    .map_err(AppError::from_repo)?;
    Ok(Json(ListEnvelope {
        items: items.iter().map(ActivityResponse::from_row).collect(),
        page,
        page_size,
        total,
    }))
}

// ============================================================================
// Integration 跨用户列表（ui-ux-risk-control §9.8）
// ============================================================================

#[derive(Debug, Deserialize)]
struct ListIntegrationsQuery {
    user_id: Option<i64>,
    /// status 过滤（§8.5 七态派生，前端过滤；后端返回全部字段，前端按 status 筛）。
    status: Option<String>,
    q: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminIntegrationResponse {
    id: i64,
    provider: String,
    user_id: i64,
    user_email: String,
    /// 派生 status（§8.5 七态全序）。
    status: String,
    mqtt_connected: bool,
    last_error: Option<String>,
    last_report_at: Option<String>,
    created_at: String,
}

impl AdminIntegrationResponse {
    fn from_row(r: &admin_service::AdminIntegrationRow) -> Self {
        let status = derive_integration_status(r);
        Self {
            id: r.id,
            provider: r.provider.clone(),
            user_id: r.user_id,
            user_email: r.user_email.clone(),
            status,
            mqtt_connected: r.mqtt_connected,
            last_error: r.last_error.clone(),
            last_report_at: r.last_report_at.and_then(|t| t.format(&Rfc3339).ok()),
            created_at: r.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}

/// 派生集成 status（§8.5 七态全序，前端后端一致）。
/// 注意：admin 列表无 agent_online 信息，agent_offline 态此处不判定（简化）。
fn derive_integration_status(r: &admin_service::AdminIntegrationRow) -> String {
    if !r.enabled {
        return "disabled".into();
    }
    if r.last_error.is_some() {
        return "error".into();
    }
    if r.last_report_at.is_none() {
        return "connecting".into();
    }
    if r.mqtt_connected {
        "connected".into()
    } else {
        "disconnected".into()
    }
}

async fn list_integrations(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListIntegrationsQuery>,
) -> AppResult<Json<ListEnvelope<AdminIntegrationResponse>>> {
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let rows = admin_service::list_all_integrations(
        &state.pool,
        q.user_id,
        q.q.as_deref(),
        page,
        page_size,
    )
    .await
    .map_err(AppError::from_repo)?;
    let total = admin_service::count_all_integrations(&state.pool, q.user_id, q.q.as_deref())
        .await
        .map_err(AppError::from_repo)?;
    // status 过滤在后端派生后做（§8.5 七态派生无法纯 SQL 过滤）。
    let items: Vec<_> = rows
        .iter()
        .map(AdminIntegrationResponse::from_row)
        .filter(|r| q.status.as_deref().is_none_or(|s| r.status == s))
        .collect();
    let total = if q.status.is_some() {
        i64::try_from(items.len()).unwrap_or(0)
    } else {
        total
    };
    Ok(Json(ListEnvelope {
        items,
        page,
        page_size,
        total,
    }))
}

// ============================================================================
// 维护模式（ui-ux-risk-control §9.10）
// ============================================================================

#[derive(Debug, Serialize)]
struct MaintenanceResponse {
    enabled: bool,
    mode: String,
    message: String,
}

async fn get_maintenance(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<MaintenanceResponse>> {
    let m = state.maintenance.snapshot();
    Ok(Json(MaintenanceResponse {
        enabled: m.enabled,
        mode: m.mode.as_str().into(),
        message: m.message,
    }))
}

#[derive(Debug, Deserialize)]
struct SetMaintenanceBody {
    enabled: bool,
    mode: String,
    #[serde(default)]
    message: Option<String>,
}

async fn set_maintenance(
    State(state): State<Arc<AppState>>,
    actor: AuthUser,
    Json(body): Json<SetMaintenanceBody>,
) -> AppResult<Json<MaintenanceResponse>> {
    let mode = crate::config::MaintenanceMode::parse(&body.mode).ok_or_else(|| {
        AppError::validation(vec![crate::error::FieldError::new("mode", "one_of")])
    })?;
    let message = body.message.unwrap_or_default();
    state
        .maintenance
        .set(body.enabled, mode, message.clone(), Some(actor.user_id))
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    // 审计：maintenance.enabled / disabled（§8.5）。
    let action = if body.enabled {
        "maintenance.enabled"
    } else {
        "maintenance.disabled"
    };
    let detail = serde_json::json!({ "mode": body.mode, "message": message });
    if let Err(e) = crate::repo::admin_repo::insert_action(
        &state.pool,
        actor.user_id,
        action,
        None,
        None,
        None,
        &detail,
    )
    .await
    {
        tracing::warn!(error = ?e, "audit {} failed", action);
    }

    let m = state.maintenance.snapshot();
    Ok(Json(MaintenanceResponse {
        enabled: m.enabled,
        mode: m.mode.as_str().into(),
        message: m.message,
    }))
}
