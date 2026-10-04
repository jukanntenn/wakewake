//! IP 封禁中间件（admin-risk-controls WRFC）。
//!
//! 挂在全局限流之外、health 之前（banned IP 不消耗 governor 预算；
//! Docker/Caddy 探活不走此层）。client-IP 口径与限流/审计一致
//! （util::client_ip_from_headers）。命中 → 403 IP_BLOCKED。
//!
//! fail-open：锁 poison / IP 不可得时放行——封禁是增强层，
//! 不能因它自身故障打死全站（优雅降级，可见记录）。

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::error::{AppError, ErrorCode};
use crate::state::AppState;
use crate::util::client_ip_from_headers;

pub async fn ip_ban_middleware(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip_from_headers(
        req.headers(),
        peer,
        state.settings.server.client_ip_header.as_deref(),
        state.settings.server.trust_proxy,
    )
    .unwrap_or_else(|| peer.ip());

    if state.ip_bans.is_banned(ip) {
        tracing::warn!(client_ip = %ip, method = %req.method(), uri = %req.uri(), "request blocked by ip ban");
        return Err(AppError::code(ErrorCode::IpBlocked));
    }
    Ok(next.run(req).await)
}
