//! Hub 派发错误（error-handling.md §9）。

/// 命令派发错误。
#[derive(thiserror::Error, Debug)]
pub enum DispatcherError {
    #[error("agent offline")]
    AgentOffline,
}
