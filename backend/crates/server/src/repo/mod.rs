//! 持久层：sqlx 查询，行映射，返回 domain 模型。
//!
//! 不含业务规则（onion-architecture）。RepoError 显式 `NotFound` + Database。

pub mod admin_repo;
pub mod agent_repo;
pub mod device_repo;
pub mod integration_repo;
pub mod refresh_token_repo;
pub mod user_repo;
pub mod wake_repo;

/// 持久层错误隔离（error-handling.md §3）。
///
/// repo 函数用 `?` 传播 `sqlx::Error`（→ Database via #[from]），
/// `fetch_optional` 返 None 时手动转 `NotFound`。
#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("row not found")]
    NotFound,
    #[error("database error")]
    Database(#[from] sqlx::Error),
}
