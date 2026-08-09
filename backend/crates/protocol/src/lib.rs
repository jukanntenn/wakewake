//! wakewake-protocol: server 与 agent 共享的 wire 类型（device-sync-v3）。
//!
//! 仅依赖 serde，纯类型定义。device-sync-v3 重构后的协议契约：
//! - **投影通道**：server→agent 用 `event: state` 全量快照（单调 `version` + `aid`），
//!   agent 仅在 `version > applied_version` 时应用，上报 ack。
//! - **command 通道**：只承载 `Wol`（唤醒，强实时）。设备/集成的增删改由 state 快照驱动。
//! - **agent 上行**：`POST /agents/self/sync`（观测值 + ack 水位）+ `POST /agents/self/wakes`
//!   （MQTT 触发的语音唤醒）+ `POST .../complete`（wake 命令回报）。
//! - 删除项：`CommandPayload::State`、`AgentReport` 枚举、`SyncState`、`DeviceSyncStatus`、
//!   `s_` 命令前缀（state-ack 占位）。agent 只上报观测值，漂移判定 100% 在 server 持久层。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// 命令 payload（server → agent，经 SSE command 事件下发）
// ============================================================================

/// SSE command 事件的 payload（id 前缀 `c_`）。
///
/// device-sync-v3 后 command 通道只承载 `Wol`（唤醒）。设备/集成的增删改全部由
/// `event: state` 全量快照驱动（agent 通过全量快照 + 单调版本感知变化）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandPayload {
    /// 唤醒：Agent RSA 解密 `mac_encrypted` → 发 magic packet → complete。
    Wol { device: WolDevice },
}

impl CommandPayload {
    /// 命令的 `"type"` 字段值（snake_case，与 serde tag 一致）。
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Wol { .. } => "wol",
        }
    }
}

// ============================================================================
// 命令 payload 引用的数据结构
// ============================================================================

/// wol 命令携带的设备（仅唤醒所需字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WolDevice {
    /// 外部标识 hex 字符串（`Uuid::simple()`，32 字符纯十六进制）。
    pub did: String,
    /// base64 RSA-OAEP+SHA-256 密文（agent 持私钥解密）。
    pub mac_encrypted: String,
}

impl WolDevice {
    /// 便捷：从 did Uuid 构造（server 侧组装 payload 时用）。
    pub fn from_did(did: Uuid, mac_encrypted: impl Into<String>) -> Self {
        Self {
            did: did.simple().to_string(),
            mac_encrypted: mac_encrypted.into(),
        }
    }
}

/// state 快照中的设备（与 create/update 设备端点同构的字段集）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceData {
    /// `Uuid::simple()` 32-hex。
    pub did: String,
    pub name: String,
    /// base64 RSA 密文（agent 持私钥解密；server 永不可见明文）。
    pub mac_encrypted: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 统一的 integration 结构 `{provider, config, enabled}`——state 快照中携带的集成配置。
///
/// config 的敏感字段（provider schema 标 `secret: true`，如 Bemfa 的 uid/secretID/secretKey）
/// 存 base64 RSA 密文，server 永不可见明文，agent 持私钥解密。非敏感字段明文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationData {
    pub provider: String,
    /// 整个 config（含敏感字段密文 + 非敏感字段明文）。JSON 动态结构，schema 驱动。
    pub config: serde_json::Value,
    #[serde(
        default = "default_enabled",
        skip_serializing_if = "is_default_enabled"
    )]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn is_default_enabled(value: &bool) -> bool {
    *value
}

// ============================================================================
// SSE state 事件 payload（全量快照，单调版本）
// ============================================================================

/// state 事件 data 的顶层结构（device-sync-v3 §8.6）。
///
/// `version` = `agents.projection_version`（单调，与 DB 写同事务自增 §5.4）；
/// agent 仅在 `version > applied_version` 时应用（I2 单调应用）。
/// `aid` = 该 agent 的 `agents.aid`（`Uuid::simple()` 32-hex），agent 据此派生 topic 命名前缀 k4（§6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub version: u64,
    pub aid: String,
    pub devices: Vec<DeviceData>,
    #[serde(default)]
    pub integrations: Vec<IntegrationData>,
}

// ============================================================================
// Agent → Server 上行
// ============================================================================

/// `POST /agents/self/commands/{command_id}/complete` 的 body（仅 wake 命令回报）。
///
/// 无论成功或失败都调 complete 端点，body 的 `success` 字段区分结果。
/// 幂等：重复 complete 或命令已过期返回 200 且不改状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

/// `POST /agents/self/sync` 的请求体（device-sync-v3 §8.6 上行 2）。
///
/// 字段可选——不同场景上报不同子集：
/// - 纯 ack（应用快照后）：`{"applied_version": 6}`
/// - reconcile 后：带上 `integration` 块（observations + repair_errors）
///
/// **agent 只上报观测值**，server 用 (历史 P, 本轮 O, 期望 D) 判定漂移（§9）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRequest {
    /// 可选：ack 水位（应用快照后的 applied_version）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_version: Option<u64>,
    /// 可选：reconcile 结果（出现 = agent 至少尝试了对账）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration: Option<SyncIntegration>,
}

/// reconcile 的集成级结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncIntegration {
    pub provider: String,
    pub enabled: bool,
    /// MQTT 实际连接状态（broker ConnAck 成功后置 true）。
    pub mqtt_connected: bool,
    /// 集成级错误（uid 解密失败等）。`null` = 无错。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// agent 查 allTopic 看到的每设备观测值。server 据此判定漂移。
    #[serde(default)]
    pub observations: Vec<Observation>,
    /// 上轮修复错误（本轮修复结果下轮报；§10.7）。
    #[serde(default)]
    pub repair_errors: Vec<RepairError>,
}

/// 单设备的观测值。`observed_name = null` 表示 topic 不存在（三态中的 ∅）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub did: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_name: Option<String>,
}

/// 上轮修复某设备时发生的错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairError {
    pub did: String,
    pub error: String,
}

/// `POST /agents/self/sync` 的响应。`current_version` 供 agent 判定孤儿删除版本守卫（I6, §10.9）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResponse {
    pub current_version: u64,
}

/// `POST /agents/self/wakes` 的请求体（MQTT on 触发的语音唤醒上报，§8.6 上行 3）。
///
/// 与 sync 字段互斥。`did` 全协议统一（不用 `device_id`）。`type=bemfa_wake` 由端点决定，不入请求体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeRequest {
    pub did: String,
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

/// `POST /agents/self/wakes` 的响应（创建持久 wake 审计记录）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeResponse {
    pub id: i64,
}

#[cfg(test)]
mod tests {
    //! 协议类型序列化往返测试——保证 server/agent 两端字段名一致。

    use super::*;

    #[test]
    fn wol_command_serializes_with_full_keys() {
        let cmd = CommandPayload::Wol {
            device: WolDevice {
                did: "550e8400".into(),
                mac_encrypted: "base64ct".into(),
            },
        };
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(json["type"], "wol");
        assert_eq!(json["device"]["did"], "550e8400");
        assert_eq!(json["device"]["mac_encrypted"], "base64ct");
        // 往返
        let back: CommandPayload = serde_json::from_value(json).unwrap();
        assert_eq!(cmd, back);
    }

    #[test]
    fn command_result_round_trip() {
        let r = CommandResult {
            success: true,
            message: "magic packet sent".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"message\":\"magic packet sent\""));
        let back: CommandResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);

        let fail = CommandResult {
            success: false,
            message: "RSA decryption failed".into(),
        };
        let fjson = serde_json::to_value(&fail).unwrap();
        assert_eq!(fjson["success"], false);
    }

    #[test]
    fn state_snapshot_round_trip_with_version_and_aid() {
        let snap = StateSnapshot {
            version: 6,
            aid: "550e8400e29b41d4a716446655440000".into(),
            devices: vec![DeviceData {
                did: "h".into(),
                name: "NAS".into(),
                mac_encrypted: "ct".into(),
                description: None,
            }],
            integrations: vec![IntegrationData {
                provider: "bemfa".into(),
                config: serde_json::json!({"uid": "ct"}),
                enabled: true,
            }],
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("\"version\":6"));
        assert!(json.contains("\"aid\":\"550e8400e29b41d4a716446655440000\""));
        let back: StateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }

    #[test]
    fn state_snapshot_empty_integrations_default() {
        // integrations 字段 default，反序列化缺失时回填空 vec
        let json = r#"{"version":1,"aid":"a","devices":[]}"#;
        let snap: StateSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snap.version, 1);
        assert!(snap.integrations.is_empty());
    }

    #[test]
    fn sync_request_pure_ack() {
        // 纯 ack：仅 applied_version
        let req = SyncRequest {
            applied_version: Some(6),
            integration: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"applied_version\":6"));
        assert!(!json.contains("integration"));
        let back: SyncRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn sync_request_with_integration_block() {
        let req = SyncRequest {
            applied_version: Some(6),
            integration: Some(SyncIntegration {
                provider: "bemfa".into(),
                enabled: true,
                mqtt_connected: true,
                error: None,
                observations: vec![
                    Observation {
                        did: "d1".into(),
                        observed_name: Some("客厅电脑".into()),
                    },
                    Observation {
                        did: "d2".into(),
                        observed_name: None, // topic 不存在
                    },
                ],
                repair_errors: vec![RepairError {
                    did: "d3".into(),
                    error: "createTopic timeout".into(),
                }],
            }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["applied_version"], 6);
        assert_eq!(json["integration"]["provider"], "bemfa");
        assert_eq!(json["integration"]["enabled"], true);
        assert_eq!(json["integration"]["mqtt_connected"], true);
        // error=null 时省略
        assert!(
            json["integration"]
                .get("error")
                .is_none_or(serde_json::Value::is_null)
        );
        assert_eq!(
            json["integration"]["observations"][0]["observed_name"],
            "客厅电脑"
        );
        assert_eq!(
            json["integration"]["observations"][1]["observed_name"],
            serde_json::Value::Null
        );
        assert_eq!(
            json["integration"]["repair_errors"][0]["error"],
            "createTopic timeout"
        );
        let back: SyncRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn sync_request_integration_with_error() {
        // 集成级错误（uid 解密失败）：error 填文案，observations 为空
        let req = SyncRequest {
            applied_version: Some(6),
            integration: Some(SyncIntegration {
                provider: "bemfa".into(),
                enabled: true,
                mqtt_connected: false,
                error: Some("uid decrypt failed".into()),
                observations: vec![],
                repair_errors: vec![],
            }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["integration"]["error"], "uid decrypt failed");
        let back: SyncRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn sync_response_round_trip() {
        let resp = SyncResponse { current_version: 7 };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"current_version\":7"));
        let back: SyncResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn wake_request_uses_did_not_device_id() {
        let req = WakeRequest {
            did: "abc".into(),
            success: true,
            message: "magic packet sent".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        // 全协议统一用 did
        assert_eq!(json["did"], "abc");
        assert!(json.get("device_id").is_none());
        let back: WakeRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn wake_response_round_trip() {
        let resp = WakeResponse { id: 123 };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"id\":123"));
        let back: WakeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn type_name_matches_tag() {
        assert_eq!(
            CommandPayload::Wol {
                device: WolDevice {
                    did: String::new(),
                    mac_encrypted: String::new()
                }
            }
            .type_name(),
            "wol"
        );
    }

    #[test]
    fn integration_default_enabled_omits_when_true() {
        // enabled=true (default) 时序列化省略，反序列化缺失时回填 true
        let i = IntegrationData {
            provider: "bemfa".into(),
            config: serde_json::json!({}),
            enabled: true,
        };
        let json = serde_json::to_string(&i).unwrap();
        assert!(!json.contains("enabled"), "json was: {json}");
        let back: IntegrationData = serde_json::from_str(&json).unwrap();
        assert!(back.enabled);

        // enabled=false 时保留
        let i2 = IntegrationData {
            enabled: false,
            ..i
        };
        let json2 = serde_json::to_string(&i2).unwrap();
        assert!(json2.contains("\"enabled\":false"));
    }
}
