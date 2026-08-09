//! integrations 表持久层（device-sync-v3 §7.0 / §7.2）。

use sqlx::PgPool;
use sqlx::types::Json;
use time::OffsetDateTime;

use crate::domain::integration::Integration;
use crate::repo::RepoError;

#[derive(sqlx::FromRow)]
struct IntegrationRow {
    id: i64,
    user_id: i64,
    agent_id: i64,
    provider: String,
    config: Json<serde_json::Value>,
    enabled: bool,
    mqtt_connected: bool,
    last_error: Option<String>,
    last_report_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl From<IntegrationRow> for Integration {
    fn from(r: IntegrationRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            agent_id: r.agent_id,
            provider: r.provider,
            config: r.config.0,
            enabled: r.enabled,
            mqtt_connected: r.mqtt_connected,
            last_error: r.last_error,
            last_report_at: r.last_report_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const COLUMNS: &str = "id, user_id, agent_id, provider, config, enabled, mqtt_connected,
                       last_error, last_report_at, created_at, updated_at";

/// 创建集成（唯一约束 `user_id+provider，冲突由` service 映射 INTEGRATION_EXISTS）。
/// 接受泛型 executor（事务支持，§5.4）。
pub async fn insert(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    user_id: i64,
    agent_id: i64,
    provider: &str,
    config: &serde_json::Value,
    enabled: bool,
) -> Result<Integration, RepoError> {
    let sql = format!(
        "INSERT INTO integrations (user_id, agent_id, provider, config, enabled)
         VALUES ($1, $2, $3, $4, $5) RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(agent_id)
        .bind(provider)
        .bind(Json(config))
        .bind(enabled)
        .fetch_one(executor)
        .await?;
    Ok(Integration::from(row))
}

/// 列用户集成（GET /integrations，走 `idx_integrations_user_provider` 最左前缀）。
pub async fn list_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<Integration>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM integrations WHERE user_id = $1 ORDER BY created_at");
    let rows = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Integration::from).collect())
}

/// 按 provider + `user_id` 查（GET/PATCH/DELETE /integrations/:provider）。
pub async fn find_for_user(
    pool: &PgPool,
    user_id: i64,
    provider: &str,
) -> Result<Option<Integration>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM integrations WHERE user_id = $1 AND provider = $2");
    let row = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(provider)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(Integration::from))
}

/// state 推送：按 `agent_id` 查全部集成（走 `idx_integrations_agent_id`）。
pub async fn list_by_agent(pool: &PgPool, agent_id: i64) -> Result<Vec<Integration>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM integrations WHERE agent_id = $1");
    let rows = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(agent_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Integration::from).collect())
}

/// 按 agent_id + provider 查（sync handler 内部用，按 agent 定位）。
pub async fn find_by_agent_provider(
    pool: &PgPool,
    agent_id: i64,
    provider: &str,
) -> Result<Option<Integration>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM integrations WHERE agent_id = $1 AND provider = $2");
    let row = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(agent_id)
        .bind(provider)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(Integration::from))
}

/// 更新 config（PATCH）。接受泛型 executor（事务支持，§5.4）。
///
/// device-sync-v3：删 sync_status 重置逻辑（全派生）。
pub async fn update_config(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    user_id: i64,
    provider: &str,
    config: Option<&serde_json::Value>,
    enabled: Option<bool>,
) -> Result<Option<Integration>, RepoError> {
    let sql = format!(
        r"UPDATE integrations
           SET config = COALESCE($3, config),
               enabled = COALESCE($4, enabled),
               updated_at = now()
           WHERE user_id = $1 AND provider = $2
           RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, IntegrationRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(provider)
        .bind(config.map(Json))
        .bind(enabled)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(Integration::from))
}

/// 删集成（DELETE）。接受泛型 executor（事务支持，§5.4）。
pub async fn delete(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    user_id: i64,
    provider: &str,
) -> Result<bool, RepoError> {
    let rows = sqlx::query("DELETE FROM integrations WHERE user_id = $1 AND provider = $2")
        .bind(user_id)
        .bind(provider)
        .execute(executor)
        .await?;
    Ok(rows.rows_affected() > 0)
}

/// 写入集成运行态（agent sync 上报时，§7.2 last_report_at 刷新口径）。
///
/// - `mqtt_connected`：MQTT 连接状态镜像（每轮覆盖）
/// - `last_error`：集成级错误（每轮覆盖；`Some(err)` 写、`None` 清空）
/// - `has_integration_block`：true = agent 完成了一轮对账（sync 报告含 `integration` 块），
///   此时刷 `last_report_at = now()`；false = 纯 ack，不刷（§7.2）
pub async fn set_runtime_status(
    pool: &PgPool,
    agent_id: i64,
    provider: &str,
    mqtt_connected: bool,
    last_error: Option<&str>,
    has_integration_block: bool,
) -> Result<(), RepoError> {
    sqlx::query(
        r"UPDATE integrations
             SET mqtt_connected = $1,
                 last_error = $2,
                 last_report_at = CASE WHEN $3 THEN now() ELSE last_report_at END,
                 updated_at = now()
           WHERE agent_id = $4 AND provider = $5",
    )
    .bind(mqtt_connected)
    .bind(last_error)
    .bind(has_integration_block)
    .bind(agent_id)
    .bind(provider)
    .execute(pool)
    .await?;
    Ok(())
}
