//! users 表持久层。

use sqlx::PgPool;
use time::OffsetDateTime;

use crate::domain::user::User;
use crate::repo::RepoError;

/// SQL 行结构（sqlx 编译期校验，字段顺序与 SELECT 一致）。
#[derive(sqlx::FromRow)]
struct UserRow {
    id: i64,
    email: String,
    password: String,
    is_active: bool,
    /// 审计列（admin disable 时写入），AdminUser 响应暴露（api-design.md §admin）。
    disabled_at: Option<OffsetDateTime>,
    is_superuser: bool,
    email_verified: bool,
    verification_sent_at: Option<OffsetDateTime>,
    last_login: Option<OffsetDateTime>,
    /// 用户偏好 locale（backend/i18n.md §5）。
    preferred_locale: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        Self {
            id: r.id,
            email: r.email,
            password: r.password,
            is_active: r.is_active,
            disabled_at: r.disabled_at,
            is_superuser: r.is_superuser,
            email_verified: r.email_verified,
            verification_sent_at: r.verification_sent_at,
            last_login: r.last_login,
            preferred_locale: r.preferred_locale,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const COLUMNS: &str = "id, email, password, is_active, disabled_at, is_superuser,
                       email_verified, verification_sent_at, last_login, preferred_locale,
                       created_at, updated_at";

/// 按 email 查（登录）。
pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM users WHERE email = $1");
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(User::from))
}

/// 按 id 查（/me）。
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<User>, RepoError> {
    let sql = format!("SELECT {COLUMNS} FROM users WHERE id = $1");
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(User::from))
}

/// 创建用户（注册）。email_verified=false（需邮件验证），is_superuser 默认 false。
/// email 唯一约束冲突由 service 层映射为 `USER_EXISTS`。
pub async fn insert(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    is_superuser: bool,
) -> Result<User, RepoError> {
    let sql = format!(
        "INSERT INTO users (email, password, is_superuser, email_verified)
         VALUES ($1, $2, $3, false) RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(email)
        .bind(password_hash)
        .bind(is_superuser)
        .fetch_one(pool)
        .await?;
    Ok(User::from(row))
}

/// 标记邮箱已验证（验证链接确认后）。
pub async fn mark_email_verified(pool: &PgPool, id: i64) -> Result<bool, RepoError> {
    let rows =
        sqlx::query("UPDATE users SET email_verified = true, updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(rows.rows_affected() > 0)
}

/// 记录验证邮件发送时间（resend 限流依据）。
pub async fn touch_verification_sent(pool: &PgPool, id: i64) -> Result<(), RepoError> {
    sqlx::query("UPDATE users SET verification_sent_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 更新密码（改密）。返回是否命中。
pub async fn update_password(
    pool: &PgPool,
    id: i64,
    password_hash: &str,
) -> Result<bool, RepoError> {
    let rows = sqlx::query("UPDATE users SET password = $1, updated_at = now() WHERE id = $2")
        .bind(password_hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(rows.rows_affected() > 0)
}

/// 更新 `last_login（登录成功`）。
pub async fn touch_last_login(pool: &PgPool, id: i64) -> Result<(), RepoError> {
    sqlx::query("UPDATE users SET last_login = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 更新 `preferred_locale（从` Accept-Language 采集，backend/i18n.md §5）。
pub async fn update_preferred_locale(
    pool: &PgPool,
    id: i64,
    locale: &str,
) -> Result<(), RepoError> {
    sqlx::query("UPDATE users SET preferred_locale = $1, updated_at = now() WHERE id = $2")
        .bind(locale)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 禁用/启用用户（admin，api-design.md §admin）。
///
/// `active=false` → `is_active=false, disabled_at=now()`；
/// `active=true` → `is_active=true, disabled_at=null`。
/// 返回更新后的行（含 `disabled_at），未命中返回` None。
pub async fn set_active(pool: &PgPool, id: i64, active: bool) -> Result<Option<User>, RepoError> {
    let sql = if active {
        format!(
            "UPDATE users SET is_active = true, disabled_at = null, updated_at = now()
             WHERE id = $1 RETURNING {COLUMNS}"
        )
    } else {
        format!(
            "UPDATE users SET is_active = false, disabled_at = now(), updated_at = now()
             WHERE id = $1 RETURNING {COLUMNS}"
        )
    };
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(User::from))
}

/// 设置 superuser 标志（management CLI promote/demote）。返回是否命中。
pub async fn set_superuser(pool: &PgPool, id: i64, value: bool) -> Result<bool, RepoError> {
    let rows = sqlx::query("UPDATE users SET is_superuser = $1, updated_at = now() WHERE id = $2")
        .bind(value)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(rows.rows_affected() > 0)
}

/// 统计 superuser 数量（demote 保底校验：至少保留 1 个 admin）。
pub async fn count_superusers(pool: &PgPool) -> Result<i64, RepoError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_superuser = true")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// 列出所有 superuser（management CLI `admin list`）。
pub async fn list_superusers(
    pool: &PgPool,
) -> Result<Vec<(i64, String, OffsetDateTime)>, RepoError> {
    let rows: Vec<(i64, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, email, created_at FROM users WHERE is_superuser = true ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 创建用户（management CLI admin create）。与 `insert` 区别：email_verified 可控。
pub async fn insert_verified(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    is_superuser: bool,
) -> Result<User, RepoError> {
    let sql = format!(
        "INSERT INTO users (email, password, is_superuser, email_verified)
         VALUES ($1, $2, $3, true) RETURNING {COLUMNS}"
    );
    let row = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(email)
        .bind(password_hash)
        .bind(is_superuser)
        .fetch_one(pool)
        .await?;
    Ok(User::from(row))
}

/// 管理员列用户（分页 + `is_active` 过滤，api-design.md §admin）。
pub async fn list_for_admin(
    pool: &PgPool,
    is_active: Option<bool>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<User>, i64), RepoError> {
    let offset = (page - 1).max(0) * page_size;
    let items: Vec<User> = if let Some(active) = is_active {
        let sql = format!(
            "SELECT {COLUMNS} FROM users WHERE is_active = $1
             ORDER BY id DESC LIMIT $2 OFFSET $3"
        );
        let rows = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(active)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        rows.into_iter().map(User::from).collect()
    } else {
        let sql = format!(
            "SELECT {COLUMNS} FROM users
             ORDER BY id DESC LIMIT $1 OFFSET $2"
        );
        let rows = sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        rows.into_iter().map(User::from).collect()
    };
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok((items, total))
}
