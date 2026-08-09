//! HTTP 路由层（axum handler）。
//!
//! routes → service（调 repo + hub）→ 构造响应。不直接查 DB（onion-architecture）。
//! 响应信封：成功单资源直接返回；列表 {`items,page,page_size,total}；错误扁平结构`。

pub mod admin;
pub mod agent_api;
pub mod agents;
pub mod auth;
pub mod commands;
pub mod devices;
pub mod health;
pub mod integrations;
pub mod sse;
pub mod user;
pub mod wakes;
