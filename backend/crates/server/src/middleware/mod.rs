//! axum 中间件（authentication.md §十）。
//!
//! 两套认证域硬分流（api-design.md §0.2）：
//! `/agents/self/*` → pairing-code `校验（agent_auth`）
//! 其余需认证端点 → user JWT 校验（auth）
//! user JWT 打进 /agents/self/* 或 pairing-code 打进浏览器端点 → 401 `AUTH_REQUIRED`。

pub mod admin_guard;
pub mod agent_auth;
pub mod auth;
pub mod locale;
pub mod rate_limit;
