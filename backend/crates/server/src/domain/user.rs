//! User domain 模型。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// 用户。email 即身份（无 name 字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    /// bcrypt $2b$ hash（永不返回给客户端）。
    #[serde(skip)]
    pub password: String,
    pub is_active: bool,
    /// 禁用时间（admin disable 时写入，enable 时清 null）。审计列，AdminUser 响应暴露。
    #[serde(with = "time::serde::rfc3339::option")]
    pub disabled_at: Option<OffsetDateTime>,
    pub is_superuser: bool,
    /// 邮箱是否已验证（注册时 FALSE，点验证链接后 TRUE）。存量用户迁移后默认 TRUE。
    pub email_verified: bool,
    /// 上次发验证邮件时间（resend 限流，防邮件轰炸）。
    #[serde(with = "time::serde::rfc3339::option")]
    pub verification_sent_at: Option<OffsetDateTime>,
    /// 兼作无状态密码重置 token 的 HMAC 输入；null 时用空串占位。
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_login: Option<OffsetDateTime>,
    /// 用户偏好 locale（异步邮件渲染用，backend/i18n.md §5）。null 时 fallback 到 "en"。
    pub preferred_locale: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// 注册新用户输入（garde 校验编译期已知字段）。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
pub struct RegisterInput {
    #[garde(email)]
    pub email: String,
    // garde length 取字面量 usize（PASSWORD_MIN_LEN=8, PASSWORD_MAX_LEN=72）。
    #[garde(length(min = 8, max = 72))]
    pub password: String,
}

use crate::domain::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN};

/// 登录输入。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
pub struct LoginInput {
    #[garde(email)]
    pub email: String,
    #[garde(length(min = 8, max = 72))]
    pub password: String,
}

/// 改密输入。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
pub struct ChangePasswordInput {
    #[garde(length(min = 8, max = 72))]
    pub current_password: String,
    #[garde(length(min = 8, max = 72))]
    pub new_password: String,
}

/// 校验密码字节数 ≤ 72（bcrypt 字节级上限）。
#[must_use]
pub fn is_password_length_valid(password: &str) -> bool {
    (PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&password.len())
}
