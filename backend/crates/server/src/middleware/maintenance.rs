//! 维护模式中间件（ui-ux-risk-control §0.4/§11.A.6）。
//!
//! 双挂载点：
//! - public_auth 域（无 JWT）：仅拦截 registration_disabled 模式的 POST /auth/register。
//! - jwt_routes 域（jwt_middleware 之后，已有 AuthUser）：拦截 readonly 写方法、full 非管理员全部。
//!
//! admin（is_superuser=true）在所有模式下放行。
//! full 模式下的 login 判断不下放中间件（无 JWT），由 auth_service::login() 处理。

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::{AppError, ErrorCode};
use crate::middleware::auth::AuthUser;
use crate::state::AppState;

/// public_auth 域维护中间件：拦截 register（所有启用模式下 register 都被拦）。
/// public_auth 域无 JWT，无法判 admin——admin 注册新账户应走 admin CLI 而非公开注册端点。
pub async fn maintenance_public(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let m = state.maintenance.snapshot();
    if !m.enabled {
        return Ok(next.run(req).await);
    }
    if is_register(&req) {
        return match m.mode {
            crate::config::MaintenanceMode::RegistrationDisabled => {
                Err(AppError::code(ErrorCode::MaintenanceRegistrationClosed))
            },
            crate::config::MaintenanceMode::Readonly => {
                Err(AppError::code(ErrorCode::MaintenanceReadonly))
            },
            crate::config::MaintenanceMode::Full => Err(AppError::code(ErrorCode::MaintenanceFull)),
        };
    }
    Ok(next.run(req).await)
}

/// jwt_routes 域维护中间件（jwt_middleware 之后，AuthUser 已可用）。
pub async fn maintenance_jwt(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let m = state.maintenance.snapshot();
    if !m.enabled {
        return Ok(next.run(req).await);
    }

    // admin 放行
    let is_admin = req
        .extensions()
        .get::<AuthUser>()
        .is_some_and(|u| u.is_superuser);
    if is_admin {
        return Ok(next.run(req).await);
    }

    match m.mode {
        crate::config::MaintenanceMode::RegistrationDisabled => {
            // jwt 域无 register 路由，放行
            Ok(next.run(req).await)
        },
        crate::config::MaintenanceMode::Readonly => {
            if is_write_method(req.method()) {
                Err(AppError::code(ErrorCode::MaintenanceReadonly))
            } else {
                Ok(next.run(req).await)
            }
        },
        crate::config::MaintenanceMode::Full => Err(AppError::code(ErrorCode::MaintenanceFull)),
    }
}

fn is_register(req: &Request) -> bool {
    req.method() == Method::POST && req.uri().path().ends_with("/auth/register")
}

fn is_write_method(m: &Method) -> bool {
    matches!(
        m,
        &Method::POST | &Method::PATCH | &Method::PUT | &Method::DELETE
    )
}
