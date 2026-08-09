//! Device domain 模型 + 创建/更新输入（device-sync-v3 §7.0）。

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// `Device`。`mac_encrypted` 全链路统一命名（DB + API + SSE）。
///
/// device-sync-v3：观测三态（`bemfa_observed_name` + `bemfa_observed_at`，§7.3）+ 漂移告警
/// （`last_drift_at`/`last_drift_kind`）+ 单设备修复错误（`last_error`）取代旧的
/// `sync_status`/`bemfa_status`（全派生，见 `domain/sync.rs`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: i64,
    pub did: Uuid,
    pub user_id: i64,
    pub agent_id: i64,
    pub name: String,
    /// base64 RSA-OAEP+SHA-256 密文（2048 位密文 base64 后 344 字符）。
    #[serde(skip)]
    pub mac_encrypted: String,
    /// 脱敏显示 AA:**:**:**:**:FF。
    pub mac_display: String,
    pub description: Option<String>,
    /// ★ 观测三态编码之一（§7.3）：name 部分。NULL 时配合 `bemfa_observed_at` 表 ⊥/∅。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bemfa_observed_name: Option<String>,
    /// ★ 观测三态编码之一：时间戳。NULL = 从未观测；有值 = 观测过。
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub bemfa_observed_at: Option<OffsetDateTime>,
    /// ★ 漂移告警时间戳（判漂移时写 now()，保留首次漂移时刻，§9.3）。
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_drift_at: Option<OffsetDateTime>,
    /// ★ 漂移告警类型：`deleted` | `renamed`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_drift_kind: Option<String>,
    /// ★ 单设备修复错误（每轮覆盖；成功 NULL，失败写错误，§9.3）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// 创建设备输入。`mac_encrypted` 由前端 RSA 加密后提交。
#[derive(Debug, Clone, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct CreateDeviceInput {
    #[garde(length(min = 1, max = 100))]
    pub name: String,
    // garde pattern 是引号字符串（feature=regex，非 pattern）。
    #[garde(pattern("^[0-9A-Za-z+/=]+$"))]
    pub mac_encrypted: String,
    /// 脱敏 MAC，格式 AA:**:**:**:**:FF。
    #[garde(length(min = 1, max = 17))]
    pub mac_display: String,
    #[garde(length(max = 255))]
    pub description: Option<String>,
    /// 可选，默认用户的默认 agent。
    pub agent_id: Option<Uuid>,
}

/// 更新设备输入（PATCH 局部更新；GitHub 主推 PATCH）。
///
/// device-sync-v3 §8.3：`mac_encrypted` 与 `mac_display` **必须成对出现**（改 MAC 时）；
/// 协议层接受 MAC 修改（密钥轮换等场景），UI 默认禁用 MAC 编辑。
#[derive(Debug, Clone, Default, Deserialize, garde::Validate)]
#[garde(allow_unvalidated)]
pub struct UpdateDeviceInput {
    #[garde(skip)]
    #[serde(default)]
    pub name: Option<String>,
    /// 仅在用户重新加密 MAC 时提供（公钥变更场景）。必须与 `mac_display` 成对。
    #[serde(default)]
    pub mac_encrypted: Option<String>,
    #[serde(default)]
    pub mac_display: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
}
