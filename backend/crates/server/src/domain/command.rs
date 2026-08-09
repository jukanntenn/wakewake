//! Command domain 模型 + 状态机（device-sync-v3 §8.4）。
//!
//! device-sync-v3 后 command 通道只承载 `Wol`（唤醒）。`s_` 前缀（state-ack 占位）已废：
//! state 快照改用单调 `version`，agent 通过 `POST /agents/self/sync` 上报 `applied_version` 做 ack。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use wakewake_protocol::CommandPayload;

/// 命令生命周期状态机：
/// pending → dispatched → completed | failed | expired。
/// 命令在内存 Hub，60s TTL，server 重启即丢（§8.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandStatus {
    /// 刚创建，未派发。
    Pending,
    /// 已通过 SSE 推给 agent。
    Dispatched,
    /// agent 回报 success=true。
    Completed,
    /// agent 回报 success=false。
    Failed,
    /// 60s 超时 agent 无响应或离线。
    Expired,
}

impl CommandStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

/// 命令结果（agent complete 回报）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub success: bool,
    pub message: String,
}

/// 内存命令（Hub 持有，不直接 JSON 序列化）。命令 ID 前缀：`c_`（command ack）。
#[derive(Debug, Clone)]
pub struct Command {
    pub id: String,
    pub status: CommandStatus,
    pub payload: CommandPayload,
    pub agent_id: i64,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub result: Option<CommandOutcome>,
}

/// 命令 id 前缀。`s_`（state-ack）已废，仅保留 `c_`（wake 命令）。
pub mod prefix {
    pub const COMMAND: &str = "c_";
}
