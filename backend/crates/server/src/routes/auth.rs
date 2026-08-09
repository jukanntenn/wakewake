//! 认证路由（api-design.md §1.4 认证端点）。
//!
//! POST /auth/register, /auth/login, /auth/refresh, /auth/password-reset/{request,confirm}。
//! 公开端点（无 JWT），限流 per-IP。password-reset/request 需 PoW（authentication.md §六）。

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::user::{LoginInput, RegisterInput};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::service::{auth_service, password_reset_service};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    routes_with_rate_limit(false)
}

/// 认证路由，可选旁路速率限制（e2e.md §2.5 `WAKEWAKE_RATE_LIMIT__DISABLED=true`）。
///
/// - register/login/refresh/pow：per-IP 10/min（authentication.md §七层1）。
/// - password-reset/*：per-IP 3/hour（更严，防邮件轰炸）。
/// - `disabled=true` 跳过 governor layer（仅旁路频率，不旁路业务配额）。
pub fn routes_with_rate_limit(disabled: bool) -> Router<Arc<AppState>> {
    let auth = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/pow/challenge", get(pow_challenge));
    let auth = crate::middleware::rate_limit::apply_auth_rate_limit(auth, disabled);

    let password_reset = Router::new()
        .route("/auth/password-reset/request", post(password_reset_request))
        .route("/auth/password-reset/confirm", post(password_reset_confirm));
    let password_reset =
        crate::middleware::rate_limit::apply_password_reset_rate_limit(password_reset, disabled);

    // 邮箱验证：verify 用 auth 限流组（与 login 同级），resend 用 password-reset 限流组（防邮件轰炸）。
    let email_verify = Router::new().route("/auth/verify-email", post(verify_email));
    let email_verify = crate::middleware::rate_limit::apply_auth_rate_limit(email_verify, disabled);

    let email_resend = Router::new().route("/auth/verify-email/resend", post(resend_verification));
    let email_resend =
        crate::middleware::rate_limit::apply_password_reset_rate_limit(email_resend, disabled);

    Router::new()
        .merge(auth)
        .merge(password_reset)
        .merge(email_verify)
        .merge(email_resend)
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: UserPublic,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: i64,
    pub email: String,
    pub is_superuser: bool,
    pub email_verified: bool,
}

impl UserPublic {
    #[must_use]
    pub fn from_user(u: &crate::domain::user::User) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            is_superuser: u.is_superuser,
            email_verified: u.email_verified,
        }
    }
}

async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<RegisterInput>,
) -> AppResult<(StatusCode, Json<UserPublic>)> {
    // 采集 Accept-Language（先于 register，使首封验证邮件用正确 locale）。
    let locale = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .map(crate::middleware::locale::parse_accept_language);
    let user = auth_service::register(
        &state.pool,
        &state.settings,
        &state.mailer,
        &input.email,
        &input.password,
    )
    .await?;
    if let Some(loc) = locale {
        let _ = crate::repo::user_repo::update_preferred_locale(&state.pool, user.id, &loc).await;
    }
    tracing::info!(user_id = user.id, email = %user.email, "user registered (pending email verification)");
    Ok((StatusCode::CREATED, Json(UserPublic::from_user(&user))))
}

#[derive(Debug, Deserialize)]
struct VerifyEmailInput {
    token: String,
}

/// 验证邮箱：校验 token → mark_email_verified → 签发 token 对（自动登录跳 dashboard）。
async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(input): Json<VerifyEmailInput>,
) -> AppResult<Json<AuthResponse>> {
    let user = crate::service::email_verification_service::verify(
        &state.pool,
        &state.settings,
        &input.token,
    )
    .await?;
    // 验证成功 → 签发 token 对（与 password-reset/confirm 对称：邮件确认后自动登录）。
    tracing::info!(user_id = user.id, email = %user.email, "email verified, auto-login");
    let tokens = issue_login_tokens(&state, user.id, &user.email, user.is_superuser).await?;
    Ok(Json(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
        user: UserPublic::from_user(&user),
    }))
}

#[derive(Debug, Deserialize)]
struct ResendVerificationInput {
    email: String,
}

/// 重发验证邮件：恒 200（防枚举 + 不泄露验证状态）。
async fn resend_verification(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ResendVerificationInput>,
) -> AppResult<Json<serde_json::Value>> {
    crate::service::email_verification_service::resend(
        &state.pool,
        &state.settings,
        &state.mailer,
        &input.email,
    )
    .await?;
    Ok(Json(json!({"sent": true})))
}

async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<LoginInput>,
) -> AppResult<Json<AuthResponse>> {
    let (user, tokens) = auth_service::login(
        &state.pool,
        &state.settings,
        &state.login_lockout,
        &input.email,
        &input.password,
    )
    .await?;
    // 采集 Accept-Language → preferred_locale（backend/i18n.md §5/§9）
    capture_preferred_locale(&state, user.id, &headers).await;
    tracing::info!(user_id = user.id, email = %user.email, "user logged in");
    Ok(Json(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
        user: UserPublic::from_user(&user),
    }))
}

/// 从 Accept-Language 头解析最佳 locale，异步写入 `users.preferred_locale`。
/// best-effort：失败不阻塞登录/注册。
async fn capture_preferred_locale(state: &Arc<AppState>, user_id: i64, headers: &HeaderMap) {
    if let Some(al) = headers.get("accept-language").and_then(|v| v.to_str().ok()) {
        let locale = crate::middleware::locale::parse_accept_language(al);
        if let Err(e) =
            crate::repo::user_repo::update_preferred_locale(&state.pool, user_id, &locale).await
        {
            tracing::warn!(error = ?e, "failed to update preferred_locale");
        }
    }
}

/// 签发 access + refresh token 对（验证成功 / 改密后自动登录复用）。
async fn issue_login_tokens(
    state: &Arc<AppState>,
    user_id: i64,
    email: &str,
    is_superuser: bool,
) -> AppResult<auth_service::AuthTokens> {
    auth_service::issue_token_pair(&state.pool, &state.settings, user_id, email, is_superuser).await
}

#[derive(Debug, Deserialize)]
struct RefreshInput {
    refresh_token: String,
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RefreshInput>,
) -> AppResult<Json<serde_json::Value>> {
    let (_, tokens) =
        auth_service::refresh(&state.pool, &state.settings, &input.refresh_token).await?;
    Ok(Json(json!({
        "access_token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
        "expires_in": tokens.expires_in,
    })))
}

#[derive(Debug, Deserialize)]
struct PasswordResetRequestInput {
    email: String,
    // PoW（authentication.md §六）：challenge + nonce（response 由 server 重算验证）。
    challenge: String,
    nonce: String,
}

/// 请求重置（含 PoW）：校验 `PoW` → 调 service → 恒 202（防枚举）。
/// mailer.enabled=false → 503 `SERVICE_UNAVAILABLE（authentication.md` §七层5）。
async fn password_reset_request(
    State(state): State<Arc<AppState>>,
    Json(input): Json<PasswordResetRequestInput>,
) -> AppResult<StatusCode> {
    // mailer 未启用 → 503 SERVICE_UNAVAILABLE（authentication.md §七层5）
    if !state.mailer.is_enabled() {
        return Err(AppError::code(ErrorCode::ServiceUnavailable));
    }
    // PoW 校验（challenge 一次性消费）
    state
        .pow
        .verify(&input.challenge, &input.nonce)
        .map_err(|_| AppError::code(ErrorCode::RateLimited))?;
    password_reset_service::request(&state.pool, &state.settings, &state.mailer, &input.email)
        .await?;
    // 恒 202（不泄露邮箱存在性，authentication.md §七层3）
    Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, Deserialize)]
struct PasswordResetConfirmInput {
    token: String,
    new_password: String,
}

/// 确认重置：验证 token → 改密 + 吊销所有 refresh → 返新 token 对（复用 issue_login_tokens）。
async fn password_reset_confirm(
    State(state): State<Arc<AppState>>,
    Json(input): Json<PasswordResetConfirmInput>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = password_reset_service::confirm(
        &state.pool,
        &state.settings,
        &input.token,
        &input.new_password,
    )
    .await?;
    let user = crate::repo::user_repo::find_by_id(&state.pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::InvalidCredentials))?;
    let tokens = issue_login_tokens(&state, user_id, &user.email, user.is_superuser).await?;
    Ok(Json(json!({
        "access_token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
        "expires_in": tokens.expires_in,
    })))
}

/// `PoW` challenge 签发（GET /pow/challenge，authentication.md §六）。
async fn pow_challenge(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let challenge = state.pow.issue();
    Ok(Json(json!({
        "id": challenge.id,
        "challenge": challenge.random_data,
        "difficulty": challenge.difficulty,
    })))
}
