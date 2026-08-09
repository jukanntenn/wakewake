//! 密码重置服务（authentication.md §四，无状态 HMAC token）。
//!
//! 借鉴 Django PasswordResetTokenGenerator：token = `base64url(user_id).base36(timestamp).truncated_hmac`，
//! HMAC 密钥派生材料含 user.password + `user.last_login` → 改密/登录自动失效（一次性免费）。
//! request：生成 token → 异步发邮件入队 → 返 202（防枚举）。
//! confirm：验证 token（HMAC + timestamp 未过期 + `password/last_login` 未变）→ 改密 + 吊销所有 refresh。

use sqlx::PgPool;

use crate::config::Settings;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::repo::user_repo;
use crate::service::mailer_service::MailerService;
use crate::service::secrets::{ResetTokenError, make_reset_token, verify_reset_token};

/// 请求重置：查 email（不存在也返 202 防枚举）→ 生成 HMAC token → 异步发邮件 → 202。
/// `PoW` 校验由调用方（route 层）完成，此处假定已通过。
pub async fn request(
    pool: &PgPool,
    settings: &Settings,
    mailer: &MailerService,
    email: &str,
) -> AppResult<()> {
    let user = user_repo::find_by_email(pool, email)
        .await
        .map_err(AppError::from_repo)?;

    if let Some(user) = user {
        let last_login_ts = user.last_login.map(time::OffsetDateTime::unix_timestamp);
        let token = make_reset_token(
            &settings.password_reset.secret,
            user.id,
            &user.password,
            last_login_ts,
        );
        // 重置链接（前端 reset-password 页带 token 查询参数）。
        // public_url 非 http(s) 时退化为相对路径（与 verify 同机制）。
        let reset_url = crate::util::build_absolute_url(
            &settings.app.public_url,
            "/reset-password",
            &[("token", &token)],
        )
        .unwrap_or_else(|| {
            tracing::warn!(
                public_url = %settings.app.public_url,
                "app.public_url is not http(s); falling back to relative reset link"
            );
            format!("/reset-password?token={token}")
        });
        // 异步发邮件（best-effort：失败记日志不影响 HTTP）。用 user.preferred_locale（backend/i18n.md §5）。
        let locale = user.preferred_locale.as_deref().unwrap_or("en");
        if let Err(e) = mailer
            .send_password_reset(&user.email, locale, &reset_url)
            .await
        {
            tracing::warn!(error = ?e, "password reset email send failed");
        }
    }
    // 无论 email 是否存在都返 202（防枚举，authentication.md §七层3）。
    Ok(())
}

/// 确认重置：验证 token → 改密（bcrypt）→ 吊销该用户所有 refresh。返回 `user_id（供签发新` token 对）。
pub async fn confirm(
    pool: &PgPool,
    settings: &Settings,
    token: &str,
    new_password: &str,
) -> AppResult<i64> {
    use crate::domain::BCRYPT_COST;
    let max_age = settings
        .password_reset
        .expire
        .parse::<humantime::Duration>()
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .as_secs() as i64;

    // 先尝试解析 token 的 user_id（需要 decode base64url + base36）。
    // 但验证需要 password_hash + last_login，需先查 user。token 含 user_id，先提取。
    // 无效/过期 token 统一返 INVALID_TOKEN(400)（偏差 G3 修复，api-design.md Part 4）。
    let user_id = extract_user_id(token).ok_or(AppError::code(ErrorCode::InvalidToken))?;
    let user = user_repo::find_by_id(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::InvalidToken))?;

    let last_login_ts = user.last_login.map(time::OffsetDateTime::unix_timestamp);
    match verify_reset_token(
        &settings.password_reset.secret,
        token,
        &user.password,
        last_login_ts,
        max_age,
    ) {
        Ok(_) => {},
        Err(ResetTokenError::Expired) => return Err(AppError::code(ErrorCode::InvalidToken)),
        Err(_) => return Err(AppError::code(ErrorCode::InvalidToken)),
    }

    // 改密（bcrypt）+ 吊销所有 refresh（强制全设备重登，authentication.md §五）。
    let new_hash = bcrypt::hash(new_password, BCRYPT_COST)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let _ = user_repo::update_password(pool, user.id, &new_hash).await;
    let _ = crate::repo::refresh_token_repo::revoke_all_for_user(pool, user.id).await;
    Ok(user.id)
}

/// 从 token 提取 `user_id（token` 第一段 base64url）。
fn extract_user_id(token: &str) -> Option<i64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let user_id_str = base64url_decode(parts[0])?;
    user_id_str.parse().ok()
}

fn base64url_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .ok()?;
    String::from_utf8(bytes).ok()
}
