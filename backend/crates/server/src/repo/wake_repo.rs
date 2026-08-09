//! wakes 审计表持久层。异步写入（buffered channel）。

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::wake::{Wake, WakeStatus, WakeType};
use crate::repo::RepoError;

#[derive(sqlx::FromRow)]
struct WakeRow {
    id: i64,
    user_id: i64,
    device_did: Uuid,
    device_name: String,
    r#type: String,
    status: String,
    message: Option<String>,
    created_at: OffsetDateTime,
}

impl From<WakeRow> for Wake {
    fn from(r: WakeRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            device_did: r.device_did,
            device_name: r.device_name,
            r#type: WakeType::parse(&r.r#type).unwrap_or(WakeType::Wol),
            status: parse_wake_status(&r.status),
            message: r.message,
            created_at: r.created_at,
        }
    }
}

const COLUMNS: &str = "id, user_id, device_did, device_name, type, status, message, created_at";

/// 写入 wake 记录（异步 buffered channel 调用）。
pub async fn insert(
    pool: &PgPool,
    user_id: i64,
    device_did: Uuid,
    device_name: &str,
    wake_type: WakeType,
    status: WakeStatus,
    message: Option<&str>,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO wakes (user_id, device_did, device_name, type, status, message)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(device_did)
    .bind(device_name)
    .bind(wake_type.as_str())
    .bind(status.as_str())
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

/// 游标分页查询（GET /wakes，?before=<`created_at`>）。
pub async fn list_for_user(
    pool: &PgPool,
    user_id: i64,
    before: Option<OffsetDateTime>,
    page_size: i64,
) -> Result<Vec<Wake>, RepoError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM wakes
          WHERE user_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2)
          ORDER BY created_at DESC LIMIT $3"
    );
    let rows = sqlx::query_as::<_, WakeRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(user_id)
        .bind(before)
        .bind(page_size)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Wake::from).collect())
}

/// 统计总数（分页信封 total）。
pub async fn count_for_user(
    pool: &PgPool,
    user_id: i64,
    before: Option<OffsetDateTime>,
) -> Result<i64, RepoError> {
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM wakes WHERE user_id = $1 AND ($2::timestamptz IS NULL OR created_at < $2)",
    )
    .bind(user_id)
    .bind(before)
    .fetch_one(pool)
    .await?;
    Ok(count.0)
}

fn parse_wake_status(s: &str) -> WakeStatus {
    match s {
        "success" => WakeStatus::Success,
        "expired" => WakeStatus::Expired,
        _ => WakeStatus::Failed,
    }
}
