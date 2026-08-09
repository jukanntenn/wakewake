//! Admin superuser 守卫中间件（api-design.md §admin）。
//!
//! 挂在 /admin/* 路由组（JWT 中间件之后）。读 `AuthUser.is_superuser`，
//! 非管理员返 **404 而非 403**（防枚举——不暴露 admin 功能存在性，
//! 与 §1.3"跨用户访问返 404 防枚举"一致）。

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::{AppError, ErrorCode};
use crate::middleware::auth::AuthUser;

/// /admin/* 守卫：非 superuser → 404（防枚举）。
pub async fn admin_guard(req: Request, next: Next) -> Result<Response, AppError> {
    let user = req
        .extensions()
        .get::<AuthUser>()
        .cloned()
        .ok_or(AppError::code(ErrorCode::AuthRequired))?;
    if !user.is_superuser {
        // 防枚举：非管理员探 /admin/* 返 404，不暴露 admin 端点存在性。
        return Err(AppError::code(ErrorCode::UserNotFound));
    }
    Ok(next.run(req).await)
}
