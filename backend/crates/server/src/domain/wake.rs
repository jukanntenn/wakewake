//! Wake 审计 domain 模型。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// 唤醒类型（wakes.type 列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeType {
    /// 手动唤醒（POST /devices/:did/wake）。
    Wol,
    /// 巴法语音触发。
    BemfaWake,
}

impl WakeType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wol => "wol",
            Self::BemfaWake => "bemfa_wake",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "wol" => Some(Self::Wol),
            "bemfa_wake" => Some(Self::BemfaWake),
            _ => None,
        }
    }
}

/// 唤醒结果状态（wakes.status 列）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeStatus {
    Success,
    Failed,
    Expired,
}

impl WakeStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

/// Wake 审计记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wake {
    pub id: i64,
    pub user_id: i64,
    /// 冗余快照（device 删除后历史仍可显示）。
    pub device_did: Uuid,
    pub device_name: String,
    pub r#type: WakeType,
    pub status: WakeStatus,
    pub message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// 写入 wake 的输入（异步 buffered channel）。
#[derive(Debug, Clone)]
pub struct RecordWake {
    pub user_id: i64,
    pub device_did: Uuid,
    pub device_name: String,
    pub r#type: WakeType,
    pub status: WakeStatus,
    pub message: Option<String>,
}
