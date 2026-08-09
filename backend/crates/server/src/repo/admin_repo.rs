//! Admin 持久层：跨用户查询 + 审计日志（admin 管理台风控）。
//!
//! 与各资源 repo 的 per-user 查询不同，这里是全局视角（admin 跨租户）。
//! 返回的行结构带 user_email（JOIN users），供 admin 表格展示归属。
//! admin_actions 审计表：每个 admin 写操作记一行（actor/action/target/detail）。

use sqlx::PgPool;
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::repo::RepoError;

// ============================================================================
// Agent 跨用户视图
// ============================================================================

/// Admin 视角的 agent 行（带归属 user email + agent 内部 id）。
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AdminAgentRow {
    pub id: i64,
    pub user_id: i64,
    pub user_email: String,
    pub aid: Uuid,
    pub name: String,
    pub has_public_key: bool,
    pub last_seen: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

const AGENT_COLUMNS: &str = "a.id, a.user_id, u.email AS user_email, a.aid, a.name,
                             (a.public_key IS NOT NULL) AS has_public_key, a.last_seen, a.created_at";

/// 列全部 agent（跨用户，offset 分页 + 可选 user_id 过滤）。
pub async fn list_all_agents(
    pool: &PgPool,
    user_id: Option<i64>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminAgentRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let rows = if let Some(uid) = user_id {
        let sql = format!(
            "SELECT {AGENT_COLUMNS} FROM agents a JOIN users u ON a.user_id = u.id
             WHERE a.user_id = $1 ORDER BY a.id DESC LIMIT $2 OFFSET $3"
        );
        sqlx::query_as::<_, AdminAgentRow>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(uid)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        let sql = format!(
            "SELECT {AGENT_COLUMNS} FROM agents a JOIN users u ON a.user_id = u.id
             ORDER BY a.id DESC LIMIT $1 OFFSET $2"
        );
        sqlx::query_as::<_, AdminAgentRow>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?
    };
    Ok(rows)
}

/// 计 agent 总数（分页信封 total，可选 user_id 过滤）。
pub async fn count_all_agents(pool: &PgPool, user_id: Option<i64>) -> Result<i64, RepoError> {
    let count: (i64,) = if let Some(uid) = user_id {
        sqlx::query_as("SELECT count(*) FROM agents WHERE user_id = $1")
            .bind(uid)
            .fetch_one(pool)
            .await?
    } else {
        sqlx::query_as("SELECT count(*) FROM agents")
            .fetch_one(pool)
            .await?
    };
    Ok(count.0)
}

// ============================================================================
// Device 跨用户视图
// ============================================================================

/// Admin 视角的 device 行（带归属 user email + agent_id + 派生 cloud_status）。
///
/// device-sync-v3：cloud_status 不再是存储列，而是从观测三态 + last_error + 期望名派生
/// （§8.3）。这里在 SQL 内用 CASE 派生，使 admin 列表与用户视图口径一致（§7.4）。
/// 无 bemfa 集成时派生为 'no_integration'（需 LEFT JOIN；此处简化为不分集成态，统一按观测态派生）。
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AdminDeviceRow {
    pub id: i64,
    pub did: Uuid,
    pub user_id: i64,
    pub user_email: String,
    pub agent_id: i64,
    pub name: String,
    pub mac_display: String,
    pub description: Option<String>,
    /// 派生 cloud_status（SQL CASE from observed_at/name/last_error）。
    pub cloud_status: String,
    pub last_error: Option<String>,
    pub last_drift_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// SQL 内 cloud_status 派生表达式（§8.3，与 domain::sync::cloud_status 同口径）。
/// 统一用 `d.` 别名（调用方 FROM 子句必须用 `devices d`），避免与 users JOIN 时的 name 歧义。
/// 注意：admin 视图不区分 no_integration（需集成态 JOIN），统一按观测态派生。
const CLOUD_STATUS_EXPR: &str = "CASE \
    WHEN d.last_error IS NOT NULL THEN 'error' \
    WHEN d.bemfa_observed_at IS NULL THEN 'not_observed' \
    WHEN d.bemfa_observed_name IS NULL OR d.bemfa_observed_name <> d.name THEN 'syncing' \
    ELSE 'synced' END";

const DEVICE_COLUMNS: &str = "d.id, d.did, d.user_id, u.email AS user_email, d.agent_id, d.name,
                              d.mac_display, d.description, d.last_error, d.last_drift_at,
                              d.created_at, d.updated_at";

/// 列全部 device（跨用户，offset 分页 + 可选 user_id / cloud_status 过滤）。
///
/// device-sync-v3：过滤维度从旧 sync_status 改为派生 cloud_status（SQL CASE）。
pub async fn list_all_devices(
    pool: &PgPool,
    user_id: Option<i64>,
    cloud_status: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminDeviceRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    // 动态 WHERE：user_id 可选；cloud_status 过滤用派生表达式（HAVING 风格，包在子查询里）。
    let sql = format!(
        "SELECT * FROM (SELECT {DEVICE_COLUMNS}, {CLOUD_STATUS_EXPR} AS cloud_status \
           FROM devices d JOIN users u ON d.user_id = u.id \
          WHERE ($1::bigint IS NULL OR d.user_id = $1)) sub \
          WHERE ($2::text IS NULL OR sub.cloud_status = $2) \
          ORDER BY sub.id DESC LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query_as::<_, AdminDeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(cloud_status)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 计 device 总数（可选 user_id / cloud_status 过滤）。
pub async fn count_all_devices(
    pool: &PgPool,
    user_id: Option<i64>,
    cloud_status: Option<&str>,
) -> Result<i64, RepoError> {
    let sql = format!(
        "SELECT count(*) FROM (SELECT {CLOUD_STATUS_EXPR} AS cs FROM devices d \
          WHERE ($1::bigint IS NULL OR d.user_id = $1)) sub \
          WHERE ($2::text IS NULL OR sub.cs = $2)"
    );
    let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(cloud_status)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}

// ============================================================================
// Wake 跨用户视图（游标分页，与 /wakes 一致）
// ============================================================================

/// Admin 视角的 wake 行（带 user_id + user_email）。
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AdminWakeRow {
    pub id: i64,
    pub user_id: i64,
    pub user_email: String,
    pub device_did: Uuid,
    pub device_name: String,
    pub r#type: String,
    pub status: String,
    pub message: Option<String>,
    pub created_at: OffsetDateTime,
}

/// 列全部 wake（跨用户，游标分页 ?before，可选 user_id 过滤）。
pub async fn list_all_wakes(
    pool: &PgPool,
    user_id: Option<i64>,
    before: Option<OffsetDateTime>,
    page_size: i64,
) -> Result<Vec<AdminWakeRow>, RepoError> {
    let rows = sqlx::query_as::<_, AdminWakeRow>(sqlx::AssertSqlSafe(
        "SELECT w.id, w.user_id, u.email AS user_email, w.device_did, w.device_name,
                w.type, w.status, w.message, w.created_at
           FROM wakes w JOIN users u ON w.user_id = u.id
          WHERE ($1::bigint IS NULL OR w.user_id = $1)
            AND ($2::timestamptz IS NULL OR w.created_at < $2)
          ORDER BY w.created_at DESC LIMIT $3",
    ))
    .bind(user_id)
    .bind(before)
    .bind(page_size)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 计 wake 总数（可选 user_id / before 过滤）。
pub async fn count_all_wakes(
    pool: &PgPool,
    user_id: Option<i64>,
    before: Option<OffsetDateTime>,
) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM wakes
          WHERE ($1::bigint IS NULL OR user_id = $1)
            AND ($2::timestamptz IS NULL OR created_at < $2)",
    )
    .bind(user_id)
    .bind(before)
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}

// ============================================================================
// 全局统计（/admin/stats）
// ============================================================================

/// 全局聚合计数（admin 概览用）。
#[derive(Debug, Clone, Default)]
pub struct GlobalStats {
    pub users: i64,
    pub active_users: i64,
    pub devices: i64,
    pub agents: i64,
    pub integrations: i64,
    pub wakes: i64,
    /// 派生 cloud_status='syncing' 的设备数（卡住资源指标，§8.3 派生口径）。
    pub devices_cloud_syncing: i64,
    /// 派生 cloud_status='error' 的设备数（异常资源指标）。
    pub devices_cloud_error: i64,
}

/// 汇总全局计数（单次并行查询）。
pub async fn global_stats(pool: &PgPool) -> Result<GlobalStats, RepoError> {
    let users: (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await?;
    let active_users: (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE is_active")
        .fetch_one(pool)
        .await?;
    let devices: (i64,) = sqlx::query_as("SELECT count(*) FROM devices")
        .fetch_one(pool)
        .await?;
    let agents: (i64,) = sqlx::query_as("SELECT count(*) FROM agents")
        .fetch_one(pool)
        .await?;
    let integrations: (i64,) = sqlx::query_as("SELECT count(*) FROM integrations")
        .fetch_one(pool)
        .await?;
    let wakes: (i64,) = sqlx::query_as("SELECT count(*) FROM wakes")
        .fetch_one(pool)
        .await?;
    // 派生 cloud_status 计数（§8.3 SQL CASE）
    let syncing_sql =
        format!("SELECT count(*) FROM devices d WHERE {CLOUD_STATUS_EXPR} = 'syncing'");
    let devices_cloud_syncing: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(syncing_sql.as_str()))
        .fetch_one(pool)
        .await?;
    let error_sql = format!("SELECT count(*) FROM devices d WHERE {CLOUD_STATUS_EXPR} = 'error'");
    let devices_cloud_error: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(error_sql.as_str()))
        .fetch_one(pool)
        .await?;
    Ok(GlobalStats {
        users: users.0,
        active_users: active_users.0,
        devices: devices.0,
        agents: agents.0,
        integrations: integrations.0,
        wakes: wakes.0,
        devices_cloud_syncing: devices_cloud_syncing.0,
        devices_cloud_error: devices_cloud_error.0,
    })
}

// ============================================================================
// 强制重同步（device-sync-v3 方案 A：bump projection_version + 断开 agent）
// ============================================================================

/// 强制重同步设备：bump 所属 agent 的 projection_version（§8 方案 A）。
/// 返回是否命中（did 存在）。
pub async fn reset_device_sync(pool: &PgPool, did: Uuid) -> Result<bool, RepoError> {
    // bump 设备所属 agent 的 projection_version（强制 version 前进让 agent 重应用 + gap 重推触发）
    let rows = sqlx::query(
        "UPDATE agents SET projection_version = projection_version + 1
          WHERE id = (SELECT agent_id FROM devices WHERE did = $1)",
    )
    .bind(did)
    .execute(pool)
    .await?;
    // 设备存在性校验：若 did 不存在，子查询返 NULL，UPDATE 影响 0 行
    let device_exists: (i64,) = sqlx::query_as("SELECT count(*) FROM devices WHERE did = $1")
        .bind(did)
        .fetch_one(pool)
        .await?;
    let _ = rows;
    Ok(device_exists.0 > 0)
}

/// 强制重同步集成：bump 所属 agent 的 projection_version（§8 方案 A）。
/// 返回是否命中。
pub async fn reset_integration_sync(pool: &PgPool, id: i64) -> Result<bool, RepoError> {
    let rows = sqlx::query(
        "UPDATE agents SET projection_version = projection_version + 1
          WHERE id = (SELECT agent_id FROM integrations WHERE id = $1)",
    )
    .bind(id)
    .execute(pool)
    .await?;
    let integ_exists: (i64,) = sqlx::query_as("SELECT count(*) FROM integrations WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let _ = rows;
    Ok(integ_exists.0 > 0)
}

/// 按 integration id 查 agent_id（resync 时定位 agent 以推 state 快照）。
pub async fn find_agent_id_by_integration(
    pool: &PgPool,
    id: i64,
) -> Result<Option<i64>, RepoError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT agent_id FROM integrations WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(agent_id,)| agent_id))
}

/// 按 device did 查 agent_id（resync 时定位 agent）。
pub async fn find_agent_id_by_device(pool: &PgPool, did: Uuid) -> Result<Option<i64>, RepoError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT agent_id FROM devices WHERE did = $1")
        .bind(did)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(agent_id,)| agent_id))
}

// ============================================================================
// admin_actions 审计表
// ============================================================================

/// 审计日志行。
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AdminActionRow {
    pub id: i64,
    pub actor_id: i64,
    pub actor_email: String,
    pub action: String,
    pub target_user_id: Option<i64>,
    pub target_agent_id: Option<i64>,
    pub target_device_did: Option<Uuid>,
    pub detail: Json<serde_json::Value>,
    pub created_at: OffsetDateTime,
}

/// 写一条审计记录。target_* 按 action 语义填一个，其余传 None。
pub async fn insert_action(
    pool: &PgPool,
    actor_id: i64,
    action: &str,
    target_user_id: Option<i64>,
    target_agent_id: Option<i64>,
    target_device_did: Option<Uuid>,
    detail: &serde_json::Value,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO admin_actions (actor_id, action, target_user_id, target_agent_id,
                                    target_device_did, detail)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor_id)
    .bind(action)
    .bind(target_user_id)
    .bind(target_agent_id)
    .bind(target_device_did)
    .bind(Json(detail))
    .execute(pool)
    .await?;
    Ok(())
}

/// 查审计日志（JOIN users 取 actor_email，offset 分页 + 可选 action 过滤）。
pub async fn list_actions(
    pool: &PgPool,
    action: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminActionRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let rows = sqlx::query_as::<_, AdminActionRow>(sqlx::AssertSqlSafe(
        "SELECT a.id, u.id AS actor_id, u.email AS actor_email, a.action,
                a.target_user_id, a.target_agent_id, a.target_device_did, a.detail, a.created_at
           FROM admin_actions a JOIN users u ON a.actor_id = u.id
          WHERE ($1::text IS NULL OR a.action = $1)
          ORDER BY a.created_at DESC LIMIT $2 OFFSET $3",
    ))
    .bind(action)
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 计审计日志总数（可选 action 过滤）。
pub async fn count_actions(pool: &PgPool, action: Option<&str>) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM admin_actions WHERE ($1::text IS NULL OR action = $1)",
    )
    .bind(action)
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}
