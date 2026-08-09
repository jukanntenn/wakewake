//! devices 表持久层（device-sync-v3 §7.0 / §9.3）。

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;
use wakewake_protocol::Observation;

use crate::domain::device::Device;
use crate::domain::sync::{DriftKind, Observed, derive_drift};
use crate::repo::RepoError;

// 允许 PgPool 用于非事务读路径（list/find），写入路径用泛型 executor 支持事务（§5.4）。

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: i64,
    did: Uuid,
    user_id: i64,
    agent_id: i64,
    name: String,
    mac_encrypted: String,
    mac_display: String,
    description: Option<String>,
    bemfa_observed_name: Option<String>,
    bemfa_observed_at: Option<OffsetDateTime>,
    last_drift_at: Option<OffsetDateTime>,
    last_drift_kind: Option<String>,
    last_error: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl From<DeviceRow> for Device {
    fn from(r: DeviceRow) -> Self {
        Self {
            id: r.id,
            did: r.did,
            user_id: r.user_id,
            agent_id: r.agent_id,
            name: r.name,
            mac_encrypted: r.mac_encrypted,
            mac_display: r.mac_display,
            description: r.description,
            bemfa_observed_name: r.bemfa_observed_name,
            bemfa_observed_at: r.bemfa_observed_at,
            last_drift_at: r.last_drift_at,
            last_drift_kind: r.last_drift_kind,
            last_error: r.last_error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const COLUMNS: &str = "id, did, user_id, agent_id, name, mac_encrypted, mac_display,
                       description, bemfa_observed_name, bemfa_observed_at,
                       last_drift_at, last_drift_kind, last_error,
                       created_at, updated_at";

/// 创建设备的输入参数（聚合，避免函数参数过多）。
pub struct NewDevice<'a> {
    pub did: Uuid,
    pub user_id: i64,
    pub agent_id: i64,
    pub name: &'a str,
    pub mac_encrypted: &'a str,
    pub mac_display: &'a str,
    pub description: Option<&'a str>,
}

/// 创建设备。新观测列全 NULL（cloud_status=not_observed，§7.1 冷启动）。
///
/// 接受泛型 executor（支持事务：与 bump_projection_version 同事务，§5.4）。
pub async fn insert(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    input: &NewDevice<'_>,
) -> Result<Device, RepoError> {
    let sql = format!(
        "INSERT INTO devices (did, user_id, agent_id, name, mac_encrypted, mac_display, description)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(input.did)
        .bind(input.user_id)
        .bind(input.agent_id)
        .bind(input.name)
        .bind(input.mac_encrypted)
        .bind(input.mac_display)
        .bind(input.description)
        .fetch_one(executor)
        .await?;
    Ok(Device::from(row))
}

/// 列用户设备（GET /devices，走 `idx_devices_user_id，禁止` N+1）。
pub async fn list_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<Device>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM devices WHERE user_id = $1 ORDER BY created_at");
    let rows = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Device::from).collect())
}

/// 按 did + `user_id` 查（跨用户访问返 None → service 映射 404 防枚举）。
pub async fn find_by_did_for_user(
    pool: &PgPool,
    did: Uuid,
    user_id: i64,
) -> Result<Option<Device>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM devices WHERE did = $1 AND user_id = $2");
    let row = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(did)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(Device::from))
}

/// state 推送：按 `agent_id` 查全部设备（走 `idx_devices_agent_id`）。
pub async fn list_by_agent(pool: &PgPool, agent_id: i64) -> Result<Vec<Device>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM devices WHERE agent_id = $1");
    let rows = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(agent_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Device::from).collect())
}

/// 配额校验（行锁防并发超额）。
/// PG 不允许 `SELECT count(*) ... FOR UPDATE`（聚合函数不允许 FOR UPDATE），
/// 改为锁定实际行再 count。接受泛型 executor（事务支持，§5.4）。
pub async fn count_for_update(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    user_id: i64,
) -> Result<i64, RepoError> {
    let rows = sqlx::query_as::<_, (i64,)>("SELECT id FROM devices WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_all(executor)
        .await?;
    Ok(rows.len() as i64)
}

/// 更新设备（PATCH 局部更新）。
///
/// device-sync-v3：删 sync_status 重置逻辑（全派生）。观测态不被用户改动影响。
/// 接受泛型 executor（事务支持，§5.4）。
pub async fn update(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    did: Uuid,
    user_id: i64,
    name: Option<&str>,
    mac_encrypted: Option<&str>,
    mac_display: Option<&str>,
    description: Option<Option<&str>>,
) -> Result<Option<Device>, RepoError> {
    let sql = format!(
        r"UPDATE devices
           SET name = COALESCE($3, name),
               mac_encrypted = COALESCE($4, mac_encrypted),
               mac_display = COALESCE($5, mac_display),
               description = CASE WHEN $6::text IS NULL THEN description ELSE $6 END,
               updated_at = now()
           WHERE did = $1 AND user_id = $2
           RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(did)
        .bind(user_id)
        .bind(name)
        .bind(mac_encrypted)
        .bind(mac_display)
        .bind(description.flatten())
        .fetch_optional(executor)
        .await?;
    Ok(row.map(Device::from))
}

/// 硬删设备（DELETE）。接受泛型 executor（事务支持，§5.4）。
pub async fn delete(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    did: Uuid,
    user_id: i64,
) -> Result<bool, RepoError> {
    let rows = sqlx::query("DELETE FROM devices WHERE did = $1 AND user_id = $2")
        .bind(did)
        .bind(user_id)
        .execute(executor)
        .await?;
    Ok(rows.rows_affected() > 0)
}

/// 按外部标识（did hex）查单设备（admin resync / wake 审计 device_name 兜底用）。
pub async fn find_by_did_hex(pool: &PgPool, did_hex: &str) -> Result<Option<Device>, RepoError> {
    let did = Uuid::parse_str(did_hex).unwrap_or_else(|_| Uuid::nil());
    find_by_did(pool, did).await
}

/// 按 did 查单设备（不限 user，admin 内部用）。
pub async fn find_by_did(pool: &PgPool, did: Uuid) -> Result<Option<Device>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM devices WHERE did = $1");
    let row = sqlx::query_as::<_, DeviceRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(did)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(Device::from))
}

/// 应用单个观测值（§9.3 事务边界三步）。
///
/// 在调用方传入的事务连接上执行：
/// 1. `SELECT ... FOR UPDATE` 取旧 P（`bemfa_observed_name` + `bemfa_observed_at`）+ 期望名 D
/// 2. 应用层用 (旧P, 新O, D) 计算 drift / kind（`derive_drift`）
/// 3. UPDATE 落库（写新 O 成为下一轮的 P）
///
/// **已删设备静默忽略**（§9.3 末尾）：UPDATE 影响 0 行不报错——该设备已不存在，观测值无意义。
/// `repair_error` 是 agent 上报的上轮修复错误（每轮覆盖；成功 NULL）。
///
/// 接受 `&mut PgConnection`（调用方从 `Transaction` 取 `connection_mut()`），保证 `Send`。
pub async fn apply_observation(
    conn: &mut sqlx::PgConnection,
    did: Uuid,
    observation: &Observation,
    repair_error: Option<&str>,
) -> Result<bool, RepoError> {
    // 1. SELECT FOR UPDATE 取旧 P + 期望名 D（事务边界，防自比自；一次查询取两列省往返）
    let prev: Option<(Option<String>, Option<OffsetDateTime>, String)> = sqlx::query_as(
        "SELECT bemfa_observed_name, bemfa_observed_at, name FROM devices WHERE did = $1 FOR UPDATE",
    )
    .bind(did)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((prev_name, prev_at, expected_name)) = prev else {
        // 设备已硬删 → 静默忽略（§9.3）
        return Ok(false);
    };

    // 2. 构造旧 P + 新 O + 派生 drift（§9.2 纯函数）
    let prev_observed = Observed::from_db(prev_at.is_some(), prev_name.as_deref());
    let new_observed = Observed::from_db(true, observation.observed_name.as_deref());
    let drift = derive_drift(&prev_observed, &new_observed, &expected_name);
    let (drift_now, drift_kind) = match drift {
        Some(DriftKind::Deleted) => (true, Some("deleted")),
        Some(DriftKind::Renamed) => (true, Some("renamed")),
        None => (false, None),
    };

    // 3. 落库（§9.3 SQL）
    let rows = sqlx::query(
        r"UPDATE devices SET
             bemfa_observed_name = $2,
             bemfa_observed_at   = now(),
             last_error          = $3,
             last_drift_at       = CASE WHEN $4 THEN now() ELSE last_drift_at END,
             last_drift_kind     = CASE WHEN $4 THEN $5 ELSE last_drift_kind END,
             updated_at          = now()
           WHERE did = $1",
    )
    .bind(did)
    .bind(&observation.observed_name)
    .bind(repair_error)
    .bind(drift_now)
    .bind(drift_kind)
    .execute(&mut *conn)
    .await?;
    Ok(rows.rows_affected() > 0)
}
