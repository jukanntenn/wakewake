//! login_events 持久层（ui-ux-risk-control §8.3）。
//!
//! 登录审计记录。写入 best-effort（失败仅 warn，不阻断登录）。
//! ip_address DB 列为 INET，Rust 侧用 String 交互（$n::inet 强制转换，避免引入 ipnetwork crate）。

use sqlx::PgPool;

use crate::repo::RepoError;

/// 写一行 login_event（ui-ux-risk-control §8.3）。
///
/// user_id 可空（撞库时 email 对应无账户）。success=false 时 failure_code 标原因。
/// ip_address 传字符串形式（如 "203.0.113.8"），SQL 内 $4::inet 转换。
/// best-effort：调用方应忽略返回错误（登录主流程不应被审计写入阻断）。
pub async fn insert(
    pool: &PgPool,
    user_id: Option<i64>,
    email: &str,
    success: bool,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
    failure_code: Option<&str>,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO login_events (user_id, email, success, ip_address, user_agent, failure_code)
         VALUES ($1, $2, $3, $4::inet, $5, $6)",
    )
    .bind(user_id)
    .bind(email)
    .bind(success)
    .bind(ip_address)
    .bind(user_agent)
    .bind(failure_code)
    .execute(pool)
    .await?;
    Ok(())
}
