//! 邮箱验证服务（注册后发验证邮件，点链接确认）。
//!
//! 复用 secrets.rs 的无状态 HMAC token（purpose="verify"，与 reset 隔离）。
//! token = base64url(user_id).base36(ts).truncated_hmac，HMAC key 含 password_hash →
//! 改密后旧验证链接自动失效（一次性，与 reset 同机制）。
//! verify：校验 token → mark_email_verified。
//! resend：限流（60s 内不重发）→ 重新生成 token 发邮件。

use sqlx::PgPool;

use crate::config::Settings;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::repo::user_repo;
use crate::service::mailer_service::MailerService;
use crate::service::secrets::{ResetTokenError, make_verify_token, verify_verify_token};

/// resend 限流间隔（同一用户 60s 内不重发，防邮件轰炸）。
const RESEND_COOLDOWN_SECS: i64 = 60;

/// 验证 token 最大有效期（24h，比 reset 的 1h 长，给用户充分确认时间）。
pub const VERIFY_TOKEN_MAX_AGE_SECS: i64 = 24 * 3600;

/// 给已注册（未验证）用户发送验证邮件：生成 token → 发邮件 → 记 verification_sent_at。
/// best-effort：邮件发送失败仅记日志（不阻塞注册 HTTP 响应）。
pub async fn send_verification_email(
    pool: &PgPool,
    settings: &Settings,
    mailer: &MailerService,
    user: &crate::domain::user::User,
) {
    let token = make_verify_token(&settings.password_reset.secret, user.id, &user.password);
    // public_url 启动时已 garde 校验为合法 URL；build_absolute_url 仅在 scheme 非 http(s) 时返 None，
    // 此时退化为相对路径（保持旧行为），并记 warn 便于发现配置异常。
    let verify_url = crate::util::build_absolute_url(
        &settings.app.public_url,
        "/verify-email",
        &[("token", &token)],
    )
    .unwrap_or_else(|| {
        tracing::warn!(
            public_url = %settings.app.public_url,
            "app.public_url is not http(s); falling back to relative verify link"
        );
        format!("/verify-email?token={token}")
    });
    let locale = user.preferred_locale.as_deref().unwrap_or("en");
    if let Err(e) = mailer
        .send_email_verification(&user.email, locale, &verify_url)
        .await
    {
        tracing::warn!(error = ?e, user_id = user.id, "verification email send failed");
    }
    let _ = user_repo::touch_verification_sent(pool, user.id).await;
}

/// 验证 token：校验 → mark_email_verified。返回验证成功的 user_id（供签发登录 token）。
pub async fn verify(
    pool: &PgPool,
    settings: &Settings,
    token: &str,
) -> AppResult<crate::domain::user::User> {
    // 先提取 user_id（token 第一段 base64url）查 user，拿到 password_hash 校验 HMAC。
    let user_id = extract_user_id(token).ok_or(AppError::code(ErrorCode::InvalidToken))?;
    let user = user_repo::find_by_id(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::InvalidToken))?;

    match verify_verify_token(
        &settings.password_reset.secret,
        token,
        &user.password,
        VERIFY_TOKEN_MAX_AGE_SECS,
    ) {
        Ok(_) => {},
        Err(ResetTokenError::Expired) => return Err(AppError::code(ErrorCode::InvalidToken)),
        Err(_) => return Err(AppError::code(ErrorCode::InvalidToken)),
    }

    // 幂等：已验证用户再次点链接也成功（不报错）。
    if !user.email_verified {
        let _ = user_repo::mark_email_verified(pool, user.id).await;
    }
    // 重新查最新状态返回（email_verified=true）。
    user_repo::find_by_id(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::InvalidToken))
}

/// 重发验证邮件（用户在 check-email 页点 resend）：限流 + 发邮件。
/// 找不到用户 / 已验证 / 限流内 统一返 EMAIL_VERIFICATION_SENT（防枚举 + 不泄露验证状态）。
pub async fn resend(
    pool: &PgPool,
    settings: &Settings,
    mailer: &MailerService,
    email: &str,
) -> AppResult<()> {
    if let Some(user) = user_repo::find_by_email(pool, email)
        .await
        .map_err(AppError::from_repo)?
    {
        // 已验证用户重发：静默成功（不泄露状态）。
        if !user.email_verified {
            // 限流：60s 内不重发
            let within_cooldown = user.verification_sent_at.is_some_and(|t| {
                (time::OffsetDateTime::now_utc() - t).whole_seconds() < RESEND_COOLDOWN_SECS
            });
            if !within_cooldown {
                send_verification_email(pool, settings, mailer, &user).await;
            }
        }
    }
    Ok(())
}

/// 从 token 提取 user_id（第一段 base64url）。
fn extract_user_id(token: &str) -> Option<i64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    base64url_decode(parts[0])?.parse().ok()
}

fn base64url_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_user_id_round_trip() {
        let secret = "s";
        let pw = "$2b$10$abc";
        let token = make_verify_token(secret, 99, pw);
        assert_eq!(extract_user_id(&token), Some(99));
    }

    #[test]
    fn extract_user_id_malformed() {
        assert_eq!(extract_user_id("not.enough"), None);
        assert_eq!(extract_user_id("a.b"), None);
        assert_eq!(extract_user_id(""), None);
    }
}
