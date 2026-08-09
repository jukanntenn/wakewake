//! Integration domain 模型（device-sync-v3 §7.0）。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Integration。config JSONB 混合存储（敏感字段密文，非敏感明文）。
///
/// device-sync-v3：`mqtt_connected`（MQTT 连接镜像）+ `last_report_at`（最近对账上报）取代
/// 旧的 `sync_status`/`last_synced_at`（全派生，见 `domain/sync.rs`）。`last_error` 保留。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Integration {
    pub id: i64,
    pub user_id: i64,
    pub agent_id: i64,
    pub provider: String,
    pub config: serde_json::Value,
    pub enabled: bool,
    /// ★ MQTT 连接状态镜像（agent ConnAck 成败上报）。
    pub mqtt_connected: bool,
    /// 集成级错误（uid 解密失败等；每轮覆盖，null 清空）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// ★ 最近一次含 `integration` 块的对账上报时间（纯 ack 不刷，§7.2）。
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_report_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// 创建集成输入。provider 决定 config schema（jsonschema 校验）。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct CreateIntegrationInput {
    #[garde(length(min = 1, max = 50))]
    pub provider: String,
    /// 动态 config，按 provider schema 校验（jsonschema，非 garde）。
    pub config: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 更新集成 config（PATCH）。
#[derive(Debug, Clone, Default, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct UpdateIntegrationInput {
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn default_true() -> bool {
    true
}

/// 把 device did 映射为外部 hex（state 推送用）。
#[must_use]
pub fn did_to_hex(did: Uuid) -> String {
    did.simple().to_string()
}
