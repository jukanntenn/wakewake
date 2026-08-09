//! 用户路由（api-design.md §1.4 用户）。JWT 域。
//!
//! GET /me；POST /me/password（改密，吊销所有 refresh）；POST /me/logout（吊销当前 refresh）。

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::domain::user::ChangePasswordInput;
use crate::error::AppResult;
use crate::middleware::auth::AuthUser;
use crate::repo::user_repo;
use crate::routes::auth::UserPublic;
use crate::service::auth_service;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/me", get(me))
        .route("/me/password", post(change_password))
        .route("/me/logout", post(logout))
}

async fn me(State(state): State<Arc<AppState>>, user: AuthUser) -> AppResult<Json<UserPublic>> {
    let u = user_repo::find_by_id(&state.pool, user.user_id)
        .await
        .map_err(crate::error::AppError::from_repo)?
        .ok_or(crate::error::AppError::code(
            crate::error::ErrorCode::InvalidCredentials,
        ))?;
    Ok(Json(UserPublic::from_user(&u)))
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(input): Json<ChangePasswordInput>,
) -> AppResult<StatusCode> {
    auth_service::change_password(
        &state.pool,
        user.user_id,
        &input.current_password,
        &input.new_password,
    )
    .await?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
struct LogoutInput {
    refresh_token: String,
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LogoutInput>,
) -> AppResult<Json<serde_json::Value>> {
    auth_service::logout(&state.pool, &input.refresh_token).await?;
    Ok(Json(json!({"ok": true})))
}
