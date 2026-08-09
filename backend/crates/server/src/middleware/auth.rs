//! JWT 校验中间件 + `AuthUser` extractor（authentication.md §十）。
//!
//! 锁 HS256 + `validate_exp` + `required_spec_claims` exp（防 alg:none）。
//! claim type 区分 access/refresh（防 refresh 当 access 用）。
//!
//! `verify_access` 通过后额外查一次 DB 的 `user.is_active（moka` 5s TTL 缓存），
//! 禁用用户最长 5s 内仍可用旧 access token，之后返 401 `USER_DISABLED`。

use std::sync::Arc;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::{AppError, ErrorCode};
use crate::repo::user_repo;
use crate::service::jwt;
use crate::state::AppState;

/// 认证后的用户身份（从 access token claims 提取）。
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub email: String,
    pub is_superuser: bool,
}

/// JWT 中间件：校验 Authorization: Bearer <`access_token`> + 查 `is_active，注入` `AuthUser`。
pub async fn jwt_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = extract_bearer(&req)?;
    let claims = jwt::verify_access(&state.settings.jwt, &token).map_err(|e| {
        // 不记 token 明文；记错误类型便于区分过期/篡改/格式错误。
        tracing::warn!(error = %e, "jwt auth failed: access token verification error");
        AppError::code(ErrorCode::TokenExpired)
    })?;

    // is_active 校验（moka 5s TTL 缓存，authentication.md §十）。
    let user_id = claims.sub;
    let is_active = check_user_active(&state, user_id).await?;
    if !is_active {
        tracing::warn!(user_id, "jwt auth failed: user disabled");
        return Err(AppError::code(ErrorCode::UserDisabled));
    }

    tracing::debug!(user_id, "jwt auth ok");
    req.extensions_mut().insert(AuthUser {
        user_id,
        email: claims.email,
        is_superuser: claims.is_superuser,
    });
    Ok(next.run(req).await)
}

/// 查 `user.is_active，命中` moka 缓存直接返回，miss 时查 DB + 写缓存。
/// 用户不存在视为 inactive（返 `UserDisabled`）。
async fn check_user_active(state: &AppState, user_id: i64) -> Result<bool, AppError> {
    if let Some(active) = state.user_active_cache.get(&user_id).await {
        return Ok(active);
    }
    let user = user_repo::find_by_id(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?;
    let active = user.is_some_and(|u| u.is_active);
    state.user_active_cache.insert(user_id, active).await;
    Ok(active)
}

/// 从 Authorization 头取 Bearer token。
fn extract_bearer(req: &Request) -> Result<String, AppError> {
    let header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::code(ErrorCode::AuthRequired))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or(AppError::code(ErrorCode::AuthRequired))?;
    Ok(token.to_string())
}

/// `AuthUser` extractor（handler 参数里直接取已认证用户）。
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| AppError::code(ErrorCode::AuthRequired).into_response())
    }
}
