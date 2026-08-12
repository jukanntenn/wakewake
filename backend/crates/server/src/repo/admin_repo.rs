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

/// 列全部 agent（跨用户，offset 分页 + 可选 user_id / email 前缀搜索，§9.7/§11.C.4）。
pub async fn list_all_agents(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminAgentRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let sql = format!(
        "SELECT {AGENT_COLUMNS} FROM agents a JOIN users u ON a.user_id = u.id
         WHERE ($1::bigint IS NULL OR a.user_id = $1)
           AND ($2::text IS NULL OR u.email ILIKE $2 || '%')
         ORDER BY a.id DESC LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query_as::<_, AdminAgentRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(q)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 计 agent 总数（可选 user_id / email 过滤）。
pub async fn count_all_agents(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM agents a JOIN users u ON a.user_id = u.id
         WHERE ($1::bigint IS NULL OR a.user_id = $1)
           AND ($2::text IS NULL OR u.email ILIKE $2 || '%')",
    )
    .bind(user_id)
    .bind(q)
    .fetch_one(pool)
    .await?;
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

/// 列全部 device（跨用户，offset 分页 + 可选 user_id / cloud_status / email 搜索，§9.6）。
///
/// device-sync-v3：过滤维度从旧 sync_status 改为派生 cloud_status（SQL CASE）。
pub async fn list_all_devices(
    pool: &PgPool,
    user_id: Option<i64>,
    cloud_status: Option<&str>,
    q: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminDeviceRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    // 动态 WHERE：user_id 可选；email/device name 前缀搜索；cloud_status 过滤用派生表达式。
    let sql = format!(
        "SELECT * FROM (SELECT {DEVICE_COLUMNS}, {CLOUD_STATUS_EXPR} AS cloud_status \
           FROM devices d JOIN users u ON d.user_id = u.id \
          WHERE ($1::bigint IS NULL OR d.user_id = $1) \
            AND ($5::text IS NULL OR u.email ILIKE $5 || '%' OR d.name ILIKE $5 || '%')) sub \
          WHERE ($2::text IS NULL OR sub.cloud_status = $2) \
          ORDER BY sub.id DESC LIMIT $3 OFFSET $4"
    );
    let rows = sqlx::query_as::<_, AdminDeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(cloud_status)
        .bind(page_size)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 计 device 总数（可选 user_id / cloud_status / email 过滤）。
pub async fn count_all_devices(
    pool: &PgPool,
    user_id: Option<i64>,
    cloud_status: Option<&str>,
    q: Option<&str>,
) -> Result<i64, RepoError> {
    let sql = format!(
        "SELECT count(*) FROM (SELECT {CLOUD_STATUS_EXPR} AS cs FROM devices d JOIN users u ON d.user_id = u.id \
          WHERE ($1::bigint IS NULL OR d.user_id = $1) \
            AND ($3::text IS NULL OR u.email ILIKE $3 || '%' OR d.name ILIKE $3 || '%')) sub \
          WHERE ($2::text IS NULL OR sub.cs = $2)"
    );
    let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(cloud_status)
        .bind(q)
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

// ============================================================================
// Activity 统一时间线（login_events UNION admin_actions，ui-ux-risk-control §0.3）
// ============================================================================

/// Activity 统一行（login 或 audit 来源，§0.3 统一 schema）。
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ActivityRow {
    pub kind: String,
    pub created_at: OffsetDateTime,
    pub actor_id: Option<i64>,
    pub actor_label: String,
    pub action: String,
    pub detail_ip: Option<String>,
    pub detail_ua: Option<String>,
    pub detail_failure_code: Option<String>,
    pub detail_target: Option<String>,
    pub detail_reason: Option<String>,
}

/// Activity 统一时间线查询（UNION ALL，offset 分页 + 多维过滤，§0.3/§11.C.5）。
///
/// 过滤维度：kind（login/audit）、result（仅 login 有效：success/failed）、
/// q（email/IP/action 关键词前缀匹配）、since/until（时间范围）。
/// UNION ALL 后 ORDER BY created_at DESC，pushed-down limit 优化。
#[allow(clippy::useless_format, clippy::too_many_arguments)]
pub async fn list_activity(
    pool: &PgPool,
    kind: Option<&str>,
    result: Option<&str>,
    q: Option<&str>,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    page: i64,
    page_size: i64,
) -> Result<Vec<ActivityRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    // 登录子查询：根据 result 过滤 success/failed
    let login_success_filter = match result {
        Some("success") => "AND success = true",
        Some("failed") => "AND success = false",
        _ => "",
    };
    // kind 过滤：None=两者都查，login=只查登录，audit=只查审计
    let (sel_login, sel_audit) = match kind {
        Some("login") => (true, false),
        Some("audit") => (false, true),
        _ => (true, true),
    };

    let mut parts: Vec<String> = Vec::new();
    if sel_login {
        parts.push(format!(
            "SELECT 'login' AS kind, created_at, user_id AS actor_id, email AS actor_label,
                    CASE WHEN success THEN 'login_success' ELSE 'login_failed' END AS action,
                    host(ip_address) AS detail_ip, user_agent AS detail_ua,
                    failure_code AS detail_failure_code, NULL::text AS detail_target, NULL::text AS detail_reason
               FROM login_events
              WHERE created_at >= $1 AND created_at <= $2
                {login_success_filter}
                AND ($5::text IS NULL OR email ILIKE $5 || '%' OR host(ip_address) ILIKE $5 || '%')"
        ));
    }
    if sel_audit {
        parts.push(format!(
            "SELECT 'audit' AS kind, a.created_at, a.actor_id, u.email AS actor_label,
                    a.action,
                    NULL::text AS detail_ip, NULL::text AS detail_ua,
                    NULL::text AS detail_failure_code,
                    CASE WHEN a.target_user_id IS NOT NULL THEN tu.email
                         WHEN a.target_agent_id IS NOT NULL THEN a.target_agent_id::text
                         WHEN a.target_device_did IS NOT NULL THEN a.target_device_did::text
                         ELSE NULL END AS detail_target,
                    a.detail->>'reason' AS detail_reason
               FROM admin_actions a
               JOIN users u ON a.actor_id = u.id
          LEFT JOIN users tu ON a.target_user_id = tu.id
              WHERE a.created_at >= $1 AND a.created_at <= $2
                AND ($5::text IS NULL OR u.email ILIKE $5 || '%' OR a.action ILIKE $5 || '%')"
        ));
    }
    let union_sql = parts.join(" UNION ALL ");
    let sql =
        format!("SELECT * FROM ({union_sql}) sub ORDER BY sub.created_at DESC LIMIT $3 OFFSET $4");

    let rows = sqlx::query_as::<_, ActivityRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(since.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        .bind(until.unwrap_or(OffsetDateTime::now_utc() + time::Duration::days(1)))
        .bind(page_size)
        .bind(offset)
        .bind(q)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Activity 总数（与 list_activity 同过滤条件）。
#[allow(clippy::useless_format, clippy::too_many_arguments)]
pub async fn count_activity(
    pool: &PgPool,
    kind: Option<&str>,
    result: Option<&str>,
    q: Option<&str>,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
) -> Result<i64, RepoError> {
    let login_success_filter = match result {
        Some("success") => "AND success = true",
        Some("failed") => "AND success = false",
        _ => "",
    };
    let (sel_login, sel_audit) = match kind {
        Some("login") => (true, false),
        Some("audit") => (false, true),
        _ => (true, true),
    };
    let mut parts: Vec<String> = Vec::new();
    if sel_login {
        parts.push(format!(
            "SELECT 1 FROM login_events
              WHERE created_at >= $1 AND created_at <= $2
                {login_success_filter}
                AND ($3::text IS NULL OR email ILIKE $3 || '%' OR host(ip_address) ILIKE $3 || '%')"
        ));
    }
    if sel_audit {
        parts.push(format!(
            "SELECT 1 FROM admin_actions a JOIN users u ON a.actor_id = u.id
              WHERE a.created_at >= $1 AND a.created_at <= $2
                AND ($3::text IS NULL OR u.email ILIKE $3 || '%' OR a.action ILIKE $3 || '%')"
        ));
    }
    let union_sql = parts.join(" UNION ALL ");
    let sql = format!("SELECT count(*) FROM ({union_sql}) sub");
    let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(since.unwrap_or(OffsetDateTime::UNIX_EPOCH))
        .bind(until.unwrap_or(OffsetDateTime::now_utc() + time::Duration::days(1)))
        .bind(q)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}

// ============================================================================
// Integration 跨用户视图（ui-ux-risk-control §9.8）
// ============================================================================

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AdminIntegrationRow {
    pub id: i64,
    pub user_id: i64,
    pub user_email: String,
    pub agent_id: i64,
    pub provider: String,
    pub enabled: bool,
    pub mqtt_connected: bool,
    pub last_error: Option<String>,
    pub last_report_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

/// 列全部 integration（跨用户，offset 分页 + user_id/status/email 过滤，§9.8）。
/// status 派生由 service 层做（§8.5 七态全序）。
pub async fn list_all_integrations(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminIntegrationRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let rows = sqlx::query_as::<_, AdminIntegrationRow>(sqlx::AssertSqlSafe(
        "SELECT i.id, i.user_id, u.email AS user_email, i.agent_id, i.provider,
                i.enabled, i.mqtt_connected, i.last_error, i.last_report_at, i.created_at
           FROM integrations i JOIN users u ON i.user_id = u.id
          WHERE ($1::bigint IS NULL OR i.user_id = $1)
            AND ($2::text IS NULL OR u.email ILIKE $2 || '%')
          ORDER BY i.id DESC LIMIT $3 OFFSET $4",
    ))
    .bind(user_id)
    .bind(q)
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_all_integrations(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(
        "SELECT count(*) FROM integrations i JOIN users u ON i.user_id = u.id
          WHERE ($1::bigint IS NULL OR i.user_id = $1)
            AND ($2::text IS NULL OR u.email ILIKE $2 || '%')",
    ))
    .bind(user_id)
    .bind(q)
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}

// ============================================================================
// Wakes offset 分页（ui-ux-risk-control §0.2，替代游标，§9.9）
// ============================================================================

/// 列全部 wake（跨用户，offset 分页 + type/result/email 过滤，§9.9）。
#[allow(clippy::too_many_arguments)]
pub async fn list_all_wakes_offset(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
    wake_type: Option<&str>,
    result: Option<&str>,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AdminWakeRow>, RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let rows = sqlx::query_as::<_, AdminWakeRow>(sqlx::AssertSqlSafe(
        "SELECT w.id, w.user_id, u.email AS user_email, w.device_did, w.device_name,
                w.type, w.status, w.message, w.created_at
           FROM wakes w JOIN users u ON w.user_id = u.id
          WHERE ($1::bigint IS NULL OR w.user_id = $1)
            AND ($2::text IS NULL OR u.email ILIKE $2 || '%' OR w.device_name ILIKE $2 || '%')
            AND ($3::text IS NULL OR w.type = $3)
            AND ($4::text IS NULL OR w.status = $4)
            AND w.created_at >= $5 AND w.created_at <= $6
          ORDER BY w.created_at DESC LIMIT $7 OFFSET $8",
    ))
    .bind(user_id)
    .bind(q)
    .bind(wake_type)
    .bind(result)
    .bind(since.unwrap_or(OffsetDateTime::UNIX_EPOCH))
    .bind(until.unwrap_or(OffsetDateTime::now_utc() + time::Duration::days(1)))
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
pub async fn count_all_wakes_offset(
    pool: &PgPool,
    user_id: Option<i64>,
    q: Option<&str>,
    wake_type: Option<&str>,
    result: Option<&str>,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(
        "SELECT count(*) FROM wakes w JOIN users u ON w.user_id = u.id
          WHERE ($1::bigint IS NULL OR w.user_id = $1)
            AND ($2::text IS NULL OR u.email ILIKE $2 || '%' OR w.device_name ILIKE $2 || '%')
            AND ($3::text IS NULL OR w.type = $3)
            AND ($4::text IS NULL OR w.status = $4)
            AND w.created_at >= $5 AND w.created_at <= $6",
    ))
    .bind(user_id)
    .bind(q)
    .bind(wake_type)
    .bind(result)
    .bind(since.unwrap_or(OffsetDateTime::UNIX_EPOCH))
    .bind(until.unwrap_or(OffsetDateTime::now_utc() + time::Duration::days(1)))
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}
