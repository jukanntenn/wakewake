//! 认证服务（authentication.md）。
//!
//! 注册/登录/刷新/重放检测/改密。bcrypt cost=10。
//! 登录时序攻击防护：用户不存在也跑一次 bcrypt hash 抹平响应时间差。
//! Refresh rotation：每次 refresh 吊销旧 + 签新；检测到已吊销 refresh 被复用 → 吊销全部。

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::config::Settings;
use crate::domain::user::User;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::repo::{refresh_token_repo, user_repo};
use crate::service::jwt;
use crate::service::mailer_service::MailerService;

/// Token 对（access + refresh 明文 + refresh hash + `expires_in`）。
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub refresh_token_hash: String,
    pub expires_in: u64,
}

/// 注册：bcrypt 哈希密码（cost=10）→ 建用户（email_verified=false）→ 建默认 agent（1:1）→ 发验证邮件。
/// 不自动登录（不发 token）：用户需点邮件链接验证邮箱后才能登录使用。
/// mailer.enabled=false 时跳过验证邮件（开发/E2E 场景，用户仍需验证，可用 resend 触发）。
pub async fn register(
    pool: &PgPool,
    settings: &Settings,
    mailer: &MailerService,
    email: &str,
    password: &str,
) -> AppResult<User> {
    use crate::domain::BCRYPT_COST;
    let hash =
        bcrypt::hash(password, BCRYPT_COST).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let user = match user_repo::insert(pool, email, &hash, false).await {
        Ok(u) => u,
        Err(crate::repo::RepoError::Database(ref e))
            if e.as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation) =>
        {
            return Err(AppError::code(ErrorCode::UserExists));
        },
        Err(e) => return Err(AppError::from_repo(e)),
    };

    // 创建默认 agent（1:1）。
    let _agent = crate::service::agent_service::create_default(pool, user.id).await?;

    // 发验证邮件（best-effort：mailer 未启用则跳过，用户仍可通过 resend 触发）。
    if mailer.is_enabled() {
        crate::service::email_verification_service::send_verification_email(
            pool, settings, mailer, &user,
        )
        .await;
    }
    Ok(user)
}

/// 登录：校验密码（常量时间）+ 抹平时序 + 失败锁定。
/// 失败返 `INVALID_CREDENTIALS。≥5` 次连续失败锁定 15min（authentication.md §八）。
pub async fn login(
    pool: &PgPool,
    settings: &Settings,
    lockout: &crate::service::login_lockout::LoginLockout,
    email: &str,
    password: &str,
) -> AppResult<(User, AuthTokens)> {
    use crate::domain::BCRYPT_COST;

    // 账号级失败锁定检查（authentication.md §八）
    if lockout.is_locked(email) {
        return Err(AppError::code(ErrorCode::RateLimited));
    }

    let user = user_repo::find_by_email(pool, email).await?;

    // 时序攻击防护：用户不存在/被禁用也跑一次 bcrypt hash 抹平响应时间差（~145ms cost=10）。
    // 区分错误码（authentication.md §三）：
    //   - 用户不存在 → INVALID_CREDENTIALS（防枚举，不暴露邮箱存在性）
    //   - 用户被禁用（is_active=false）→ USER_DISABLED（让用户明确"账号问题，联系管理员"）
    //     仍跑 bcrypt 抹平时序，避免"返 USER_DISABLED 快 / 返 INVALID_CREDENTIALS 慢"的侧信道。
    let user = match user {
        Some(u) if u.is_active => u,
        Some(_) => {
            let _ = bcrypt::hash(password, BCRYPT_COST);
            let _ = lockout.record_failure(email);
            return Err(AppError::code(ErrorCode::UserDisabled));
        },
        None => {
            let _ = bcrypt::hash(password, BCRYPT_COST);
            let _ = lockout.record_failure(email);
            return Err(AppError::code(ErrorCode::InvalidCredentials));
        },
    };

    let valid = bcrypt::verify(password, &user.password).unwrap_or(false);
    if !valid {
        let _ = lockout.record_failure(email);
        return Err(AppError::code(ErrorCode::InvalidCredentials));
    }

    // 邮箱未验证：拦截登录，引导用户完成验证流程（不清除失败计数，防止枚举侧信道）。
    if !user.email_verified {
        return Err(AppError::code(ErrorCode::EmailNotVerified));
    }

    // 成功 → 清除失败计数（authentication.md §八）
    lockout.record_success(email);

    let _ = user_repo::touch_last_login(pool, user.id).await;
    let tokens = issue_token_pair(pool, settings, user.id, &user.email, user.is_superuser).await?;
    Ok((user, tokens))
}

/// 刷新 access + refresh rotation + 重放检测（authentication.md §二）。
pub async fn refresh(
    pool: &PgPool,
    settings: &Settings,
    refresh_token: &str,
) -> AppResult<(i64, AuthTokens)> {
    let claims = jwt::verify_refresh(&settings.jwt, refresh_token)
        .map_err(|_| AppError::code(ErrorCode::TokenExpired))?;
    let user_id = claims.sub;

    let hash = jwt::hash_token(refresh_token);
    let row = refresh_token_repo::find_by_hash(pool, &hash)
        .await
        .map_err(AppError::from_repo)?;

    match row {
        Some(r) if r.revoked_at.is_none() && r.expires_at > OffsetDateTime::now_utc() => {
            // 正常轮转：吊销旧 refresh（先 revoke 防重放）
            let _ = refresh_token_repo::revoke(pool, &hash).await;
            let user = user_repo::find_by_id(pool, user_id)
                .await
                .map_err(AppError::from_repo)?
                .ok_or(AppError::code(ErrorCode::InvalidCredentials))?;
            // is_active 校验（authentication.md §二）：revoke 后、issue 前。
            // 顺序重要：禁用用户的 refresh 既被吊销了又没拿到新 token。
            if !user.is_active {
                return Err(AppError::code(ErrorCode::UserDisabled));
            }
            let tokens =
                issue_token_pair(pool, settings, user_id, &user.email, user.is_superuser).await?;
            Ok((user_id, tokens))
        },
        Some(_) => {
            // 已吊销的 refresh 被复用 → token theft，吊销该用户所有 refresh。
            tracing::warn!(user_id, "refresh token reuse detected");
            let _ = refresh_token_repo::revoke_all_for_user(pool, user_id).await;
            // 偏差 H4 修复：禁用用户的 refresh 在禁用时已被批量吊销，会落到此分支而非
            // line 120 的 is_active 检查。优先返回 USER_DISABLED，让前端给出"账号已禁用"
            // 而非误导性的"token 过期"（authentication.md §二）。
            if let Ok(Some(u)) = user_repo::find_by_id(pool, user_id).await {
                if !u.is_active {
                    return Err(AppError::code(ErrorCode::UserDisabled));
                }
            }
            Err(AppError::code(ErrorCode::TokenExpired))
        },
        None => Err(AppError::code(ErrorCode::TokenExpired)),
    }
}

/// 登出：吊销当前 refresh。
pub async fn logout(pool: &PgPool, refresh_token: &str) -> AppResult<()> {
    let hash = jwt::hash_token(refresh_token);
    let _ = refresh_token_repo::revoke(pool, &hash).await;
    Ok(())
}

/// 改密：bcrypt 新密码 + 吊销该用户所有 refresh（强制全设备重登）。
pub async fn change_password(
    pool: &PgPool,
    user_id: i64,
    current_password: &str,
    new_password: &str,
) -> AppResult<()> {
    use crate::domain::BCRYPT_COST;
    let user = user_repo::find_by_id(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::InvalidCredentials))?;

    if !bcrypt::verify(current_password, &user.password).unwrap_or(false) {
        return Err(AppError::code(ErrorCode::InvalidCredentials));
    }

    let new_hash = bcrypt::hash(new_password, BCRYPT_COST)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let _ = user_repo::update_password(pool, user_id, &new_hash).await;
    let _ = refresh_token_repo::revoke_all_for_user(pool, user_id).await;
    Ok(())
}

/// 签发 access + refresh token 对，并持久化 refresh hash。
pub async fn issue_token_pair(
    pool: &PgPool,
    settings: &Settings,
    user_id: i64,
    email: &str,
    is_superuser: bool,
) -> AppResult<AuthTokens> {
    let access = jwt::issue_access(&settings.jwt, user_id, email, is_superuser)
        .map_err(AppError::Internal)?;
    let refresh_plain = jwt::issue_refresh(&settings.jwt, user_id).map_err(AppError::Internal)?;
    let refresh_hash = jwt::hash_token(&refresh_plain);
    let refresh_ttl = settings.refresh_ttl().map_err(AppError::Internal)?;
    let expires_in = settings.access_ttl().map_err(AppError::Internal)?.as_secs();

    // 持久化 refresh hash（明文永不入库）
    refresh_token_repo::insert(
        pool,
        user_id,
        &refresh_hash,
        OffsetDateTime::now_utc() + Duration::seconds(refresh_ttl.as_secs() as i64),
    )
    .await
    .map_err(AppError::from_repo)?;

    Ok(AuthTokens {
        access_token: access,
        refresh_token: refresh_plain,
        refresh_token_hash: refresh_hash,
        expires_in,
    })
}
