//! Agent domain 模型 + 三态枚举。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Agent 三态（api-design.md §2.13 / §5.7）。
/// pending：无 `public_key；online：有` `public_key` + SSE 连接在；offline：有 `public_key` + SSE 断。
///
/// 与 `sync_status` 正交——agent 在线状态是 SSE 连接状态（纯内存），
/// `sync_status` 是资源同步状态（持久化在 DB）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// 无 `public_key，等待` agent 首次连接。UI 显示完整 `pairing_code`。
    Pending,
    /// 有 `public_key` + SSE `连接在。绿点，pairing_code` 脱敏。
    Online,
    /// 有 `public_key` + SSE `断。灰点，pairing_code` 脱敏。
    Offline,
}

impl AgentStatus {
    /// 由 `public_key` 是否存在 + SSE 是否在线推导三态。
    #[must_use]
    pub fn classify(has_public_key: bool, sse_connected: bool) -> Self {
        match (has_public_key, sse_connected) {
            (false, _) => Self::Pending,
            (true, true) => Self::Online,
            (true, false) => Self::Offline,
        }
    }
}

/// Agent。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: i64,
    pub user_id: i64,
    pub aid: Uuid,
    pub name: String,
    /// 终身凭证，16 hex。永不记日志（或 ****1234）。
    #[serde(skip)]
    pub pairing_code: String,
    /// PEM，nullable（pending 状态）。
    pub public_key: Option<String>,
    /// ★ 投影单调版本（与数据写同事务自增，§5.4；ack 比对 + gap 重推依据）。
    pub projection_version: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_seen: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
