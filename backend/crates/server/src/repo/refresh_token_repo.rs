//! `refresh_tokens` 表持久层。
//!
//! rotation + 重放检测（authentication.md §`二）。token_hash` 存 SHA-256(明文) hex，明文永不入库。

use sqlx::PgPool;
use time::OffsetDateTime;

use crate::repo::RepoError;

#[derive(sqlx::FromRow)]
pub struct RefreshTokenRow {
    pub id: i64,
    pub user_id: i64,
    pub token_hash: String,
    pub expires_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

/// 插入新 refresh token。
pub async fn insert(
    pool: &PgPool,
    user_id: i64,
    token_hash: &str,
    expires_at: OffsetDateTime,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 按 `token_hash` 查（含 `revoked_at` 状态，供 rotation + 重放检测判断）。
pub async fn find_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshTokenRow>, RepoError> {
    let row = sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, revoked_at, created_at
           FROM refresh_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 吊销单个 token（rotation / logout）。
pub async fn revoke(pool: &PgPool, token_hash: &str) -> Result<(), RepoError> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// 吊销该用户全部有效 refresh（改密 / 检测到重放）。
pub async fn revoke_all_for_user(pool: &PgPool, user_id: i64) -> Result<u64, RepoError> {
    let rows = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now()
          WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(rows.rows_affected())
}

/// 清理过期行（定时任务，每日）。
pub async fn delete_expired(pool: &PgPool) -> Result<u64, RepoError> {
    let rows = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now()")
        .execute(pool)
        .await?;
    Ok(rows.rows_affected())
}
