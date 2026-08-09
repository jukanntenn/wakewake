//! 管理员服务（api-design.md §admin + authentication.md §十）。
//!
//! 风控第一版：管理员能通过普通用户无法触达的操作，把数据/程序纠正回正确状态。
//! 每个写操作都写一条 admin_actions 审计记录（actor/action/target/detail）。
//!
//! 已有能力：禁用/启用用户 + 联动副作用（吊销 refresh、断开 SSE）。
//! 新增能力：
//! - reset_password：管理员设新密码（绕过旧密码校验）+ 吊销所有 refresh
//! - verify_email：强制标记邮箱已验证
//! - resync_device / resync_integration：重置 sync_status + 断开 agent 触发重连重推 state
//! - disconnect_agent：强制断开 agent SSE（清理僵尸连接）
//! - get_stats：全局聚合计数
//! - get_audit_log：审计日志查询
//! - list_all_agents / list_all_devices / list_all_wakes：跨用户资源列表

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::user::User;
use crate::error::{AppError, AppResult, ErrorCode, FieldError};
use crate::hub::CommandDispatcher;
use crate::repo::{admin_repo, agent_repo, refresh_token_repo, user_repo};
use crate::state::AppState;

// ============================================================================
// 审计写入辅助：每个 admin 写操作收尾时记一行。
// 错误不阻塞主流程（审计失败仅记日志，不回滚业务操作）。
// ============================================================================

async fn audit(
    pool: &PgPool,
    actor_id: i64,
    action: &str,
    target_user_id: Option<i64>,
    target_agent_id: Option<i64>,
    target_device_did: Option<Uuid>,
    detail: &serde_json::Value,
) {
    if let Err(e) = admin_repo::insert_action(
        pool,
        actor_id,
        action,
        target_user_id,
        target_agent_id,
        target_device_did,
        detail,
    )
    .await
    {
        tracing::warn!(error = ?e, %action, "admin audit log write failed");
    }
}

// ============================================================================
// 用户管理
// ============================================================================

/// 禁用用户（api-design.md §admin disable 联动副作用）：
/// 1. `UPDATE users SET is_active=false, disabled_at=now()`。
/// 2. 吊销该用户所有 `refresh_token（revoke_all_for_user`）。
/// 3. 强制断开该用户 agent 的活跃 SSE 连接（hub.unsubscribe）。
///
/// 状态机：对已禁用用户再次 disable 返 409 SYNCING（幂等拒绝，api-design.md §admin）。
/// 已签发的 access token（15min TTL）失效由 JWT 中间件 `is_active` 查询兜底（5s moka 缓存）。
pub async fn disable_user(state: &AppState, actor_id: i64, user_id: i64) -> AppResult<User> {
    // 状态机校验：当前已禁用 → 409 SYNCING（幂等拒绝）
    let current = user_repo::find_by_id(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::UserNotFound))?;
    if !current.is_active {
        return Err(AppError::code(ErrorCode::Syncing));
    }

    // 1. 写 is_active=false + disabled_at
    let user = user_repo::set_active(&state.pool, user_id, false)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::UserNotFound))?;

    // 2. 吊销该用户所有 refresh_token
    let _ = refresh_token_repo::revoke_all_for_user(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?;

    // 3. 断开 agent SSE 连接（1:1 模型，按 user_id 找 agent_id）
    //    agent 收到流结束按既定退避重连，重连时 pairing_code 中间件 JOIN users
    //    查到 is_active=false → 401 USER_DISABLED → agent 立即终止（不重连）。
    if let Some(agent_id) = agent_repo::find_agent_id_by_user(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?
    {
        state.hub.unsubscribe(agent_id);
    }

    // 失效 is_active moka 缓存（让 JWT 中间件下次查询走 DB，5s 内不必等）
    state.user_active_cache.invalidate(&user_id).await;

    audit(
        &state.pool,
        actor_id,
        "user.disable",
        Some(user_id),
        None,
        None,
        &serde_json::json!({}),
    )
    .await;

    Ok(user)
}

/// 启用用户（api-design.md §admin enable）：
/// 仅 `UPDATE users SET is_active=true, disabled_at=null`。**不自动重新签发 token**——
/// 用户需重新登录（与改密后行为一致）。agent 需用户重启（或重新配对）。
///
/// 状态机：对已启用用户再次 enable 返 409 SYNCING。
pub async fn enable_user(state: &AppState, actor_id: i64, user_id: i64) -> AppResult<User> {
    let current = user_repo::find_by_id(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::UserNotFound))?;
    if current.is_active {
        return Err(AppError::code(ErrorCode::Syncing));
    }

    let user = user_repo::set_active(&state.pool, user_id, true)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::UserNotFound))?;

    state.user_active_cache.invalidate(&user_id).await;

    audit(
        &state.pool,
        actor_id,
        "user.enable",
        Some(user_id),
        None,
        None,
        &serde_json::json!({}),
    )
    .await;

    Ok(user)
}

/// 管理员重置用户密码（绕过旧密码校验）。
/// 1. bcrypt 哈希新密码（cost=10）。
/// 2. UPDATE users SET password=新哈希。
/// 3. 吊销该用户所有 refresh_token（强制全设备重登，与改密后行为一致）。
///
/// 故障模式：用户忘密 / 邮箱不可达求助。管理员凭 superuser 身份直接设新密码。
pub async fn reset_password(
    state: &AppState,
    actor_id: i64,
    user_id: i64,
    new_password: &str,
) -> AppResult<()> {
    use crate::domain::{BCRYPT_COST, user::is_password_length_valid};

    // 校验新密码长度（NIST 8-72，bcrypt 字节上限 72）。
    if !is_password_length_valid(new_password) {
        return Err(AppError::validation(vec![FieldError::new(
            "new_password",
            if new_password.len() < 8 {
                "min_length"
            } else {
                "max_length"
            },
        )]));
    }

    // 确认用户存在（不存在 → 404 防枚举一致）。
    let _ = user_repo::find_by_id(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::UserNotFound))?;

    let new_hash = bcrypt::hash(new_password, BCRYPT_COST)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let updated = user_repo::update_password(&state.pool, user_id, &new_hash)
        .await
        .map_err(AppError::from_repo)?;
    if !updated {
        return Err(AppError::code(ErrorCode::UserNotFound));
    }

    // 吊销所有 refresh（强制全设备重登）。
    let _ = refresh_token_repo::revoke_all_for_user(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?;

    audit(
        &state.pool,
        actor_id,
        "user.reset_password",
        Some(user_id),
        None,
        None,
        &serde_json::json!({}),
    )
    .await;

    Ok(())
}

/// 强制标记用户邮箱已验证。
/// 故障模式：邮件服务故障导致用户卡在未验证状态无法登录。
pub async fn verify_email(state: &AppState, actor_id: i64, user_id: i64) -> AppResult<()> {
    let updated = user_repo::mark_email_verified(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?;
    if !updated {
        return Err(AppError::code(ErrorCode::UserNotFound));
    }

    audit(
        &state.pool,
        actor_id,
        "user.verify_email",
        Some(user_id),
        None,
        None,
        &serde_json::json!({}),
    )
    .await;

    Ok(())
}

/// 列用户（分页 + `is_active` 过滤，GET /admin/users）。
pub async fn list_users(
    pool: &PgPool,
    is_active: Option<bool>,
    page: i64,
    page_size: i64,
) -> AppResult<(Vec<User>, i64)> {
    let (items, total) = user_repo::list_for_admin(pool, is_active, page, page_size).await?;
    Ok((items, total))
}

// ============================================================================
// 跨用户资源列表（admin 视角）
// ============================================================================

pub use admin_repo::{
    AdminActionRow, AdminAgentRow, AdminDeviceRow, AdminWakeRow, GlobalStats, count_actions,
    count_all_agents, count_all_devices, count_all_wakes, global_stats, list_actions,
    list_all_agents, list_all_devices, list_all_wakes,
};

// ============================================================================
// 强制重同步（resync）
// ============================================================================

/// 强制重同步设备：把 sync_status 置 syncing + 断开 agent 触发重连重推 state 快照。
/// agent 离线时仅置 syncing，agent 重连时 SSE 握手本就推全量 state。
///
/// 故障模式：设备 sync_status 卡在 syncing/sync_error（agent 漏 ack / 部分失败）。
pub async fn resync_device(state: &AppState, actor_id: i64, did: Uuid) -> AppResult<()> {
    let updated = admin_repo::reset_device_sync(&state.pool, did)
        .await
        .map_err(AppError::from_repo)?;
    if !updated {
        return Err(AppError::code(ErrorCode::DeviceNotFound));
    }

    // 定位 agent：在线则断开触发重连（重连握手推全量 state）。
    if let Some(agent_id) = admin_repo::find_agent_id_by_device(&state.pool, did)
        .await
        .map_err(AppError::from_repo)?
    {
        state.hub.unsubscribe(agent_id);
    }

    audit(
        &state.pool,
        actor_id,
        "device.resync",
        None,
        None,
        Some(did),
        &serde_json::json!({}),
    )
    .await;

    Ok(())
}

/// 强制重同步集成：把 sync_status 置 syncing + 断开 agent 触发重连。
/// 故障模式：集成 sync_status 卡住（agent 漏 ack / 集成侧副作用未完成）。
pub async fn resync_integration(
    state: &AppState,
    actor_id: i64,
    integration_id: i64,
) -> AppResult<()> {
    let updated = admin_repo::reset_integration_sync(&state.pool, integration_id)
        .await
        .map_err(AppError::from_repo)?;
    if !updated {
        return Err(AppError::code(ErrorCode::IntegrationNotFound));
    }

    if let Some(agent_id) = admin_repo::find_agent_id_by_integration(&state.pool, integration_id)
        .await
        .map_err(AppError::from_repo)?
    {
        state.hub.unsubscribe(agent_id);
    }

    audit(
        &state.pool,
        actor_id,
        "integration.resync",
        None,
        None,
        None,
        &serde_json::json!({ "integration_id": integration_id }),
    )
    .await;

    Ok(())
}

// ============================================================================
// 强制断开 agent SSE
// ============================================================================

/// 强制断开 agent SSE 连接（清理僵尸/卡死连接）。
/// agent 收到流结束按既定退避重连；重连时如用户已禁用则终止。
/// 对离线 agent 调用是 no-op（hub 里本就无该 agent）。
///
/// 故障模式：hub 标记 agent online 但实际进程卡死/网络僵尸。
pub async fn disconnect_agent(state: &AppState, actor_id: i64, agent_id: i64) -> AppResult<()> {
    // 确认 agent 存在（不存在 → 404）。
    let _ = agent_repo::find_by_id(&state.pool, agent_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    state.hub.unsubscribe(agent_id);

    audit(
        &state.pool,
        actor_id,
        "agent.disconnect",
        None,
        Some(agent_id),
        None,
        &serde_json::json!({}),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn reset_password_rejects_short_password() {
        // 校验逻辑：new_password.len() < 8 → min_length 字段错误。
        // 业务函数需 DB，这里只验证 is_password_length_valid 的边界。
        use crate::domain::user::is_password_length_valid;
        assert!(!is_password_length_valid("short"));
        assert!(is_password_length_valid("8charstr"));
        // 73 字节超 bcrypt 上限。
        let too_long = "a".repeat(73);
        assert!(!is_password_length_valid(&too_long));
    }
}
