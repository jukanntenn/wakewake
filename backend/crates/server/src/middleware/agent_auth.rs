//! Agent pairing-code 校验中间件（/agents/self/* 域，api-design.md §0.2）。
//!
//! pairing-code 不透明随机串（16 hex），server 查库校验（不用 JWT）。
//! 注入 `AuthAgent（agent_id`, `user_id）到` request extensions。

use std::sync::Arc;

use axum::extract::{FromRequestParts, Request};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use crate::error::{AppError, ErrorCode};
use crate::repo::agent_repo;

/// 认证后的 agent 身份。
#[derive(Debug, Clone)]
pub struct AuthAgent {
    pub agent_id: i64,
    pub user_id: i64,
}

/// pairing-code 中间件：校验 Authorization: Bearer <`pairing_code`> + JOIN users 校验 `is_active`。
///
/// `find_by_pairing_code` 已 JOIN users 取 is_active（api-design.md §2.4）。
/// 禁用用户的 agent 即便 `pairing_code` 仍有效也无法连入 → 401 `USER_DISABLED`。
pub async fn pairing_code_middleware(
    pool: axum::extract::State<Arc<PgPool>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("agent auth failed: missing Authorization header");
            AppError::code(ErrorCode::AuthRequired)
        })?;
    let pairing_code = header.strip_prefix("Bearer ").ok_or_else(|| {
        tracing::warn!("agent auth failed: Authorization header is not Bearer scheme");
        AppError::code(ErrorCode::AuthRequired)
    })?;

    let agent = agent_repo::find_by_pairing_code(&pool, pairing_code)
        .await
        .map_err(AppError::from_repo)?
        .ok_or_else(|| {
            // 脱敏：只记长度，不记 pairing_code 明文（防日志泄露凭证）。
            tracing::warn!(
                code_len = pairing_code.len(),
                "agent auth failed: pairing code not found in DB"
            );
            AppError::code(ErrorCode::AuthRequired)
        })?;

    // is_active 校验（SSE 是长连接，建立时查一次 DB 可接受，不用 moka 缓存）。
    if !agent.is_active() {
        tracing::warn!(agent_id = agent.id, "agent auth failed: user disabled");
        return Err(AppError::code(ErrorCode::UserDisabled));
    }

    tracing::debug!(agent_id = agent.id, "agent auth ok");
    req.extensions_mut().insert(AuthAgent {
        agent_id: agent.id,
        user_id: agent.user_id,
    });
    Ok(next.run(req).await)
}

impl<S> FromRequestParts<S> for AuthAgent
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthAgent>()
            .cloned()
            .ok_or_else(|| AppError::code(ErrorCode::AuthRequired).into_response())
    }
}
