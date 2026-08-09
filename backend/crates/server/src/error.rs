//! 分层错误契约（error-handling.md）。
//!
//! AppError（应用层统一错误）+ ErrorCode（自带 HTTP 映射）+ FieldError（嵌套字段路径）。
//! service 调 repo 后用 match 转业务码；handler 调 `AppError::into_response()` 构造扁平信封。
//! 零 panic：clippy -W `unwrap_used` -W `expect_used` 从源头消除。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;

use crate::repo::RepoError;

// ---- FieldError（error-handling.md §4.1）----

/// 字段级错误。field 是点分隔路径（"name" / "config.uid" / "devices.0.mac"）。
#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: Cow<'static, str>,
    /// 泛化规则码：required / `invalid_format` / `min_length` / `max_length` / `limit_exceeded` ...
    pub code: &'static str,
    /// 规则参数（如 min/max），前端 i18n 模板插值用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<&'static str, String>>,
}

impl FieldError {
    pub fn new(field: impl Into<Cow<'static, str>>, code: &'static str) -> Self {
        Self {
            field: field.into(),
            code,
            params: None,
        }
    }
}

// ---- ErrorCode（自带 HTTP 映射，error-handling.md §4.2 + api-design.md Part 4）----

/// `业务错误码。UPPER_SNAKE` 机器码（前端据此分支，绝不依赖 message 文本）。
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ErrorCode {
    #[error("authentication required")]
    AuthRequired, // 401 AUTH_REQUIRED
    #[error("token expired")]
    TokenExpired, // 401 TOKEN_EXPIRED
    #[error("invalid credentials")]
    InvalidCredentials, // 401 INVALID_CREDENTIALS
    #[error("user disabled")]
    UserDisabled, // 401 USER_DISABLED（用户被管理员禁用，api-design.md Part 4）
    #[error("user already exists")]
    UserExists, // 409 USER_EXISTS
    #[error("email not verified")]
    EmailNotVerified, // 403 EMAIL_NOT_VERIFIED（未验证邮箱用户尝试登录，引导验证流程）
    #[error("quota exceeded")]
    QuotaExceeded, // 422 QUOTA_EXCEEDED
    #[error("device not found")]
    DeviceNotFound, // 404 DEVICE_NOT_FOUND
    #[error("agent offline")]
    AgentOffline, // 202 AGENT_OFFLINE
    #[error("integration not found")]
    IntegrationNotFound, // 404 INTEGRATION_NOT_FOUND
    #[error("provider not found")]
    ProviderNotFound, // 404 PROVIDER_NOT_FOUND
    #[error("agent not found")]
    AgentNotFound, // 404（GET /agents/default 无 agent 时）
    #[error("user not found")]
    UserNotFound, // 404 USER_NOT_FOUND（admin 操作不存在用户）
    #[error("resource syncing")]
    Syncing, // 409 SYNCING
    #[error("integration already exists")]
    IntegrationExists, // 409 INTEGRATION_EXISTS（device-sync-v3 §8.5：重复创建集成）
    #[error("rate limited")]
    RateLimited, // 429 RATE_LIMITED
    #[error("invalid or expired token")]
    InvalidToken, // 400 INVALID_TOKEN（密码重置 token 无效/过期，api-design.md Part 4）
    #[error("service unavailable")]
    ServiceUnavailable, // 503 SERVICE_UNAVAILABLE（mailer disabled 等）
    #[error("internal error")]
    Internal, // 500 INTERNAL_ERROR
}

impl ErrorCode {
    /// `UPPER_SNAKE` 机器码（进 ErrorResponse.code）。
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::UserDisabled => "USER_DISABLED",
            Self::UserExists => "USER_EXISTS",
            Self::EmailNotVerified => "EMAIL_NOT_VERIFIED",
            Self::QuotaExceeded => "QUOTA_EXCEEDED",
            Self::DeviceNotFound => "DEVICE_NOT_FOUND",
            Self::AgentOffline => "AGENT_OFFLINE",
            Self::IntegrationNotFound => "INTEGRATION_NOT_FOUND",
            Self::ProviderNotFound => "PROVIDER_NOT_FOUND",
            Self::AgentNotFound => "AGENT_NOT_FOUND",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::Syncing => "SYNCING",
            Self::IntegrationExists => "INTEGRATION_EXISTS",
            Self::RateLimited => "RATE_LIMITED",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            Self::Internal => "INTERNAL_ERROR",
        }
    }

    /// HTTP 状态码。
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::AuthRequired
            | Self::TokenExpired
            | Self::InvalidCredentials
            | Self::UserDisabled => StatusCode::UNAUTHORIZED,
            Self::DeviceNotFound
            | Self::IntegrationNotFound
            | Self::ProviderNotFound
            | Self::AgentNotFound
            | Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::UserExists | Self::Syncing | Self::IntegrationExists => StatusCode::CONFLICT,
            Self::EmailNotVerified => StatusCode::FORBIDDEN,
            Self::InvalidToken => StatusCode::BAD_REQUEST,
            Self::QuotaExceeded => StatusCode::UNPROCESSABLE_ENTITY,
            // 202：不阻塞命令创建，命令将 60s expired（api-design.md Part 4）。
            Self::AgentOffline => StatusCode::ACCEPTED,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// 英文兜底 message（前端不翻译时的人读 fallback）。
    #[must_use]
    pub fn fallback_message(&self) -> &'static str {
        match self {
            Self::AuthRequired => "Authentication required",
            Self::TokenExpired => "Token expired",
            Self::InvalidCredentials => "Invalid email or password",
            Self::UserDisabled => "User account is disabled",
            Self::UserExists => "User already exists",
            Self::EmailNotVerified => "Email not verified",
            Self::QuotaExceeded => "Quota exceeded",
            Self::DeviceNotFound => "Device not found",
            Self::AgentOffline => "Agent is offline",
            Self::IntegrationNotFound => "Integration not found",
            Self::ProviderNotFound => "Provider not found",
            Self::AgentNotFound => "Agent not found",
            Self::UserNotFound => "User not found",
            Self::Syncing => "Resource is syncing",
            Self::IntegrationExists => "Integration already exists",
            Self::RateLimited => "Too many requests",
            Self::InvalidToken => "Invalid or expired token",
            Self::ServiceUnavailable => "Service unavailable",
            Self::Internal => "Internal server error",
        }
    }
}

// ---- AppError（应用层统一错误，error-handling.md §4）----

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("validation failed")]
    Validation { field_errors: Vec<FieldError> },

    #[error("{0}")]
    Code(ErrorCode),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    #[must_use]
    pub fn code(ec: ErrorCode) -> Self {
        Self::Code(ec)
    }

    #[must_use]
    pub fn validation(field_errors: Vec<FieldError>) -> Self {
        Self::Validation { field_errors }
    }

    /// 把 `RepoError` 转为 Internal（意外错误），由 service 在已知上下文另作映射。
    #[must_use]
    pub fn from_repo(e: RepoError) -> Self {
        match e {
            RepoError::NotFound => Self::Internal(anyhow::anyhow!("repo: row not found")),
            RepoError::Database(e) => Self::Internal(e.into()),
        }
    }
}

// RepoError::NotFound → service 显式映射为具体业务码（如 DeviceNotFound），
// 不在这里隐式转 Internal。From 仅处理 Database（意外）。
impl From<RepoError> for AppError {
    fn from(e: RepoError) -> Self {
        Self::from_repo(e)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.into())
    }
}

// ---- IntoResponse（handler 边界，error-handling.md §6）----

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            Self::Code(ec) => {
                let body = Json(serde_json::json!({
                    "code": ec.code(),
                    "message": ec.fallback_message(),
                }));
                (ec.status(), body).into_response()
            },
            Self::Validation { field_errors } => {
                let body = Json(serde_json::json!({
                    "code": "VALIDATION_FAILED",
                    "message": "Validation failed",
                    "errors": field_errors,
                }));
                (StatusCode::UNPROCESSABLE_ENTITY, body).into_response()
            },
            Self::Internal(err) => {
                // 详细写日志，客户端只见通用 500（不泄露内部细节）。
                tracing::error!(error = ?err, "internal error");
                let body = Json(serde_json::json!({
                    "code": "INTERNAL_ERROR",
                    "message": "Internal server error",
                }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            },
        }
    }
}

/// Result 别名（handler/service 惯用）。
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_status_mapping() {
        assert_eq!(ErrorCode::AuthRequired.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(ErrorCode::DeviceNotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(ErrorCode::AgentOffline.status(), StatusCode::ACCEPTED);
        assert_eq!(
            ErrorCode::QuotaExceeded.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            ErrorCode::RateLimited.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            ErrorCode::Internal.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_code_machine_names() {
        assert_eq!(ErrorCode::AgentOffline.code(), "AGENT_OFFLINE");
        assert_eq!(ErrorCode::QuotaExceeded.code(), "QUOTA_EXCEEDED");
        assert_eq!(ErrorCode::ProviderNotFound.code(), "PROVIDER_NOT_FOUND");
    }
}
