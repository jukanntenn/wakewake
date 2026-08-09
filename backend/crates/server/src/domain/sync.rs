//! 派生状态 + 漂移判定纯函数（device-sync-v3 §7.4 / §8.3 / §8.5 / §9）。
//!
//! 派生逻辑集中此处，供 `routes/devices.rs`、`routes/admin.rs`、`routes/integrations.rs`、
//! sync handler 共用，避免 UI 与 admin 两套口径（§7.4）。
//!
//! **零 IO**——所有函数是纯函数，输入已读的 DB 值，输出派生态。漂移判定用旧 P
//! （§9.3 事务边界：调用方在同事务内 SELECT FOR UPDATE 取旧 P 后再调本模块）。

use serde::{Deserialize, Serialize};

// ============================================================================
// 观测三态（§7.3 / §9.1）
// ============================================================================

/// 三态观测值（P = 历史观测，O = 本轮观测）。
///
/// - `NeverObserved`(⊥)：从未观测（`bemfa_observed_at IS NULL`，新设备/集成刚启用）
/// - `TopicMissing`(∅)：观测过但 topic 不存在（`observed_at` 有值 + `observed_name IS NULL`）
/// - `Name(name)`：观测过且 topic 存在（`observed_at` 有值 + `observed_name` 有值）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    NeverObserved,
    TopicMissing,
    Name(String),
}

impl Observed {
    /// 从 DB 列构造（observed_at 为 Option<OffsetDateTime>，observed_name 为 Option<String>）。
    #[must_use]
    pub fn from_db(observed_at_is_some: bool, observed_name: Option<&str>) -> Self {
        match (observed_at_is_some, observed_name) {
            (false, _) => Self::NeverObserved,
            (true, None) => Self::TopicMissing,
            (true, Some(name)) => Self::Name(name.to_string()),
        }
    }
}

// ============================================================================
// 设备 cloud_status 派生（§8.3）
// ============================================================================

/// 设备云同步状态（§8.3 GET /devices 响应的 `cloud_status` 字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudStatus {
    /// 无 bemfa 集成 或 集成 disabled。
    NoIntegration,
    /// 从未观测（`cloud_observed_at IS NULL`）。
    NotObserved,
    /// 观测≠期望，正在修复（`observed ≠ name ∧ last_error IS NULL`）。
    Syncing,
    /// 观测==期望（`observed == name ∧ last_error IS NULL`）。
    Synced,
    /// 修复失败（`last_error IS NOT NULL`）。
    Error,
}

impl CloudStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoIntegration => "no_integration",
            Self::NotObserved => "not_observed",
            Self::Syncing => "syncing",
            Self::Synced => "synced",
            Self::Error => "error",
        }
    }
}

/// 派生 `cloud_status`（§8.3 精确布尔式）。
///
/// 输入：
/// - `integration_present`：是否有 bemfa 集成
/// - `enabled`：集成是否启用
/// - `observed_at_is_some`：`bemfa_observed_at` 是否有值
/// - `observed_name`：`bemfa_observed_name`（NULL = topic 不存在）
/// - `expected_name`：期望昵称 = `devices.name`
/// - `last_error`：单设备修复错误（非空 = 修复失败）
#[must_use]
pub fn cloud_status(
    integration_present: bool,
    enabled: bool,
    observed_at_is_some: bool,
    observed_name: Option<&str>,
    expected_name: &str,
    last_error: Option<&str>,
) -> CloudStatus {
    // §8.3 精确布尔式（按文档顺序）：
    if !integration_present || !enabled {
        return CloudStatus::NoIntegration;
    }
    if last_error.is_some() {
        return CloudStatus::Error;
    }
    if !observed_at_is_some {
        return CloudStatus::NotObserved;
    }
    // observed_at 有值：判观测==期望
    // observed_name IS NULL（topic 不存在）必然 ≠ 期望名（字符串）；
    // observed_name <> devices.name 即观测≠期望。OR 合并 = 观测≠期望。
    let observed_eq_expected = observed_name.is_some_and(|n| n == expected_name);
    if observed_eq_expected {
        CloudStatus::Synced
    } else {
        CloudStatus::Syncing
    }
}

// ============================================================================
// 投影 projection_status 派生（§8.3）
// ============================================================================

/// 投影同步状态（§8.3 GET /devices 响应的 `projection_status` 字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatus {
    /// agent 不在线。
    AgentOffline,
    /// agent 在线但 `acked < projection_version`。
    Syncing,
    /// `acked ≥ projection_version`。
    Synced,
}

impl ProjectionStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentOffline => "agent_offline",
            Self::Syncing => "syncing",
            Self::Synced => "synced",
        }
    }
}

/// 派生 `projection_status`（§8.3）。
///
/// 输入：
/// - `agent_online`：设备所属 agent 在 hub 连接表是否存在（§8.3 agent_online 兜底）
/// - `acked`：hub 的 `last_acked_version`
/// - `projection_version`：`agents.projection_version`
#[must_use]
pub fn projection_status(
    agent_online: bool,
    acked: i64,
    projection_version: i64,
) -> ProjectionStatus {
    if !agent_online {
        return ProjectionStatus::AgentOffline;
    }
    if acked >= projection_version {
        ProjectionStatus::Synced
    } else {
        ProjectionStatus::Syncing
    }
}

// ============================================================================
// 漂移判定（§9.2 纯函数）
// ============================================================================

/// 漂移类型（§9.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// 云端 topic 被删（`O == ∅`）。
    Deleted,
    /// 云端 topic 被改名（`O` 是异名字符串）。
    Renamed,
}

impl DriftKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
        }
    }
}

/// 漂移判定纯函数（§9.2）。
///
/// ```text
/// drift = (P ≠ ⊥) ∧ (O ≠ P) ∧ (O ≠ D)
/// kind  = if O == ∅ then "deleted" else "renamed"
/// ```
///
/// 字符串相等比较按**字节精确匹配**（§9.2：默认按字节，实测显示名一般原样往返）。
/// 事务边界（§9.3）：`prev` 必须是 UPDATE 之前的旧 P，由调用方在同事务内 SELECT FOR UPDATE 取。
#[must_use]
pub fn derive_drift(prev: &Observed, new_observed: &Observed, expected: &str) -> Option<DriftKind> {
    // 分支 1：P == ⊥（从未观测）→ 不漂移
    if *prev == Observed::NeverObserved {
        return None;
    }
    // 分支 2：O == P（观测没变）→ 不漂移
    if new_observed == prev {
        return None;
    }
    // 分支 3：O == D（观测到期望值）→ 不漂移
    if let Observed::Name(name) = new_observed {
        if name == expected {
            return None;
        }
    }
    // 分支 4：其余（P 有值，O 既不等于 P 也不等于 D）→ 漂移
    match new_observed {
        Observed::TopicMissing => Some(DriftKind::Deleted),
        // O 是异名字符串（O≠P 且 O≠D）
        Observed::Name(_) => Some(DriftKind::Renamed),
        // O == ⊥（本轮从未观测）：P 有值但 O 是 ⊥。规范真值表未显式列此组合，
        // 但 §9.2 公式 O≠P∧O≠D 对 O=⊥ 成立（⊥≠任何有值 P，⊥≠字符串 D）。
        // 这表示"上次观测过、这次 agent 没上报该设备"——理论上 agent observations
        // 含全部投影设备，不应漏报；若发生，按"观测缺失"保守判为非漂移（不告警，
        // 等下一轮完整观测）。返回 None。
        Observed::NeverObserved => None,
    }
}

// ============================================================================
// 集成 status 派生（§8.5）
// ============================================================================

/// 集成运行态（§8.5 GET /integrations 响应的 `status` 字段，7 值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    /// 集成不存在。
    NotConnected,
    /// agent 不在线（优先于一切运行时状态）。
    AgentOffline,
    /// enabled=false（用户意图优先于运行态）。
    Disabled,
    /// last_error 非空（配置错误/解密失败等）。
    Error,
    /// last_report_at 为空（配了但 agent 还没上报过 integration 块）。
    Connecting,
    /// last_report_at 有值、mqtt_connected=false、无错。
    Disconnected,
    /// mqtt_connected=true、无错。
    Connected,
}

impl IntegrationStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotConnected => "not_connected",
            Self::AgentOffline => "agent_offline",
            Self::Disabled => "disabled",
            Self::Error => "error",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Connected => "connected",
        }
    }
}

/// 派生集成 `status`（§8.5 严格全序优先级，匹配即返回）。
///
/// 输入：
/// - `integration_present`：集成行是否存在
/// - `agent_online`：agent 是否在线
/// - `enabled`：用户意图 enabled 布尔
/// - `last_error`：集成级错误
/// - `last_report_at_is_some`：是否上报过 integration 块
/// - `mqtt_connected`：MQTT 连接状态镜像
#[must_use]
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn integration_status(
    integration_present: bool,
    agent_online: bool,
    enabled: bool,
    last_error: Option<&str>,
    last_report_at_is_some: bool,
    mqtt_connected: bool,
) -> IntegrationStatus {
    // §8.5 全序优先级（匹配即返回）：
    if !integration_present {
        return IntegrationStatus::NotConnected;
    }
    if !agent_online {
        return IntegrationStatus::AgentOffline;
    }
    if !enabled {
        return IntegrationStatus::Disabled;
    }
    if last_error.is_some() {
        return IntegrationStatus::Error;
    }
    if !last_report_at_is_some {
        return IntegrationStatus::Connecting;
    }
    if !mqtt_connected {
        return IntegrationStatus::Disconnected;
    }
    IntegrationStatus::Connected
}

#[cfg(test)]
mod tests {
    //! 漂移判定真值表穷举（§9.4）+ 派生状态优先级（§8.3 / §8.5）。

    use super::*;

    // ---- Observed::from_db 三态 ----

    #[test]
    fn observed_from_db_three_states() {
        // ⊥ 从未观测
        assert_eq!(Observed::from_db(false, None), Observed::NeverObserved);
        assert_eq!(Observed::from_db(false, Some("x")), Observed::NeverObserved);
        // ∅ topic 不存在
        assert_eq!(Observed::from_db(true, None), Observed::TopicMissing);
        // 字符串值
        assert_eq!(
            Observed::from_db(true, Some("客厅")),
            Observed::Name("客厅".into())
        );
    }

    // ---- derive_drift 真值表 §9.4 全 11 种情形 ----

    const X: &str = "X";
    const Y: &str = "Y";

    #[test]
    fn case_1_new_device_no_topic_no_drift() {
        // 新设备，尚未创建：P=⊥ O=∅ D=X
        let d = derive_drift(&Observed::NeverObserved, &Observed::TopicMissing, X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_2_new_device_cloud_same_name_no_drift() {
        // 新设备，云端已存在同名：P=⊥ O=X D=X
        let d = derive_drift(&Observed::NeverObserved, &Observed::Name(X.into()), X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_3_new_device_cloud_diff_name_no_drift() {
        // 新设备，云端存在异名：P=⊥ O=Y D=X（server 生成 did 正常不可达，modifyName 改正）
        let d = derive_drift(&Observed::NeverObserved, &Observed::Name(Y.into()), X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_4_create_failed_retry_no_drift() {
        // create 失败后重试：P=∅ O=∅ D=X（O==P）
        let d = derive_drift(&Observed::TopicMissing, &Observed::TopicMissing, X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_5_create_success_no_drift() {
        // create 成功：P=∅ O=X D=X（O==D 收敛）
        let d = derive_drift(&Observed::TopicMissing, &Observed::Name(X.into()), X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_6_steady_state_no_drift() {
        // 稳态：P=X O=X D=X（O==P==D）
        let d = derive_drift(&Observed::Name(X.into()), &Observed::Name(X.into()), X);
        assert_eq!(d, None);
    }

    #[test]
    fn case_7_user_deleted_cloud_topic_drifts_deleted() {
        // 用户删除云端 topic：P=X O=∅ D=X → 漂移 deleted
        let d = derive_drift(&Observed::Name(X.into()), &Observed::TopicMissing, X);
        assert_eq!(d, Some(DriftKind::Deleted));
    }

    #[test]
    fn case_8_user_renamed_cloud_topic_drifts_renamed() {
        // 用户改名云端 topic：P=X O=Y D=X → 漂移 renamed
        let d = derive_drift(&Observed::Name(X.into()), &Observed::Name(Y.into()), X);
        assert_eq!(d, Some(DriftKind::Renamed));
    }

    #[test]
    fn case_9_self_rename_success_no_drift() {
        // 我方改名成功：P=X O=X' D=X'（O==D 已收敛）
        let d = derive_drift(
            &Observed::Name(X.into()),
            &Observed::Name("X'".into()),
            "X'",
        );
        assert_eq!(d, None);
    }

    #[test]
    fn case_10_self_rename_failed_retry_no_drift() {
        // 我方改名失败后重试：P=X O=X D=X'（O==P 改名没生效）
        let d = derive_drift(&Observed::Name(X.into()), &Observed::Name(X.into()), "X'");
        assert_eq!(d, None);
    }

    #[test]
    fn case_11_user_renamed_in_ui_no_drift() {
        // 用户在 UI 改名（合法）：P=X O=X D=X'（O==P 云端没变，期望变了）
        let d = derive_drift(&Observed::Name(X.into()), &Observed::Name(X.into()), "X'");
        assert_eq!(d, None);
    }

    #[test]
    fn drift_kind_deleted_vs_renamed_str() {
        assert_eq!(DriftKind::Deleted.as_str(), "deleted");
        assert_eq!(DriftKind::Renamed.as_str(), "renamed");
    }

    #[test]
    fn drift_observed_missing_this_round_no_drift() {
        // P 有值、O=⊥（本轮 agent 漏报该设备）：保守非漂移
        let d = derive_drift(&Observed::Name(X.into()), &Observed::NeverObserved, X);
        assert_eq!(d, None);
    }

    // ---- cloud_status 派生 §8.3 ----

    #[test]
    fn cloud_status_no_integration() {
        // 无集成
        assert_eq!(
            cloud_status(false, true, true, Some("x"), "x", None),
            CloudStatus::NoIntegration
        );
        // 集成 disabled
        assert_eq!(
            cloud_status(true, false, true, Some("x"), "x", None),
            CloudStatus::NoIntegration
        );
    }

    #[test]
    fn cloud_status_error_when_last_error() {
        // last_error 非空（优先级高于 observed 判定）
        assert_eq!(
            cloud_status(true, true, true, Some("x"), "x", Some("err")),
            CloudStatus::Error
        );
        // 即使 observed≠期望，last_error 优先 → error
        assert_eq!(
            cloud_status(true, true, true, Some("y"), "x", Some("err")),
            CloudStatus::Error
        );
    }

    #[test]
    fn cloud_status_not_observed() {
        // 从未观测
        assert_eq!(
            cloud_status(true, true, false, None, "x", None),
            CloudStatus::NotObserved
        );
    }

    #[test]
    fn cloud_status_syncing_when_observed_ne_expected() {
        // 观测≠期望（topic 不存在 ∅）
        assert_eq!(
            cloud_status(true, true, true, None, "x", None),
            CloudStatus::Syncing
        );
        // 观测≠期望（异名）
        assert_eq!(
            cloud_status(true, true, true, Some("y"), "x", None),
            CloudStatus::Syncing
        );
    }

    #[test]
    fn cloud_status_synced_when_observed_eq_expected() {
        // 观测==期望
        assert_eq!(
            cloud_status(true, true, true, Some("x"), "x", None),
            CloudStatus::Synced
        );
    }

    // ---- projection_status 派生 §8.3 ----

    #[test]
    fn projection_status_offline_when_agent_offline() {
        assert_eq!(
            projection_status(false, 5, 5),
            ProjectionStatus::AgentOffline
        );
        // 即使 acked>=version，agent 离线优先
        assert_eq!(
            projection_status(false, 10, 5),
            ProjectionStatus::AgentOffline
        );
    }

    #[test]
    fn projection_status_synced_when_acked_ge_version() {
        assert_eq!(projection_status(true, 5, 5), ProjectionStatus::Synced);
        assert_eq!(projection_status(true, 6, 5), ProjectionStatus::Synced);
    }

    #[test]
    fn projection_status_syncing_when_acked_lt_version() {
        assert_eq!(projection_status(true, 4, 5), ProjectionStatus::Syncing);
        assert_eq!(projection_status(true, 0, 5), ProjectionStatus::Syncing);
    }

    // ---- integration_status 派生 §8.5 全序优先级 ----

    #[test]
    fn integration_status_not_connected_when_absent() {
        assert_eq!(
            integration_status(false, true, true, None, true, true),
            IntegrationStatus::NotConnected
        );
    }

    #[test]
    fn integration_status_agent_offline_priority() {
        // agent 离线优先于一切运行时状态（即使 enabled=false、有错）
        assert_eq!(
            integration_status(true, false, false, Some("e"), false, false),
            IntegrationStatus::AgentOffline
        );
    }

    #[test]
    fn integration_status_disabled_when_not_enabled() {
        // disabled 优先于运行态
        assert_eq!(
            integration_status(true, true, false, Some("e"), false, false),
            IntegrationStatus::Disabled
        );
    }

    #[test]
    fn integration_status_error_when_last_error() {
        // error（last_error 非空）优先于 connecting/disconnected/connected
        assert_eq!(
            integration_status(true, true, true, Some("e"), false, false),
            IntegrationStatus::Error
        );
        assert_eq!(
            integration_status(true, true, true, Some("e"), true, true),
            IntegrationStatus::Error
        );
    }

    #[test]
    fn integration_status_connecting_when_never_reported() {
        // connecting：last_report_at 为空（从未上报 integration 块）
        assert_eq!(
            integration_status(true, true, true, None, false, false),
            IntegrationStatus::Connecting
        );
        // 即使 mqtt_connected=true（理论上未上报不该有 mqtt_connected=true，但派生按字段）
        assert_eq!(
            integration_status(true, true, true, None, false, true),
            IntegrationStatus::Connecting
        );
    }

    #[test]
    fn integration_status_disconnected_when_reported_but_mqtt_down() {
        // disconnected：last_report_at 有值、mqtt_connected=false、无错
        assert_eq!(
            integration_status(true, true, true, None, true, false),
            IntegrationStatus::Disconnected
        );
    }

    #[test]
    fn integration_status_connected_when_mqtt_up() {
        // connected：mqtt_connected=true、无错、已上报
        assert_eq!(
            integration_status(true, true, true, None, true, true),
            IntegrationStatus::Connected
        );
    }

    #[test]
    fn integration_status_as_str_all_seven() {
        assert_eq!(IntegrationStatus::NotConnected.as_str(), "not_connected");
        assert_eq!(IntegrationStatus::AgentOffline.as_str(), "agent_offline");
        assert_eq!(IntegrationStatus::Disabled.as_str(), "disabled");
        assert_eq!(IntegrationStatus::Error.as_str(), "error");
        assert_eq!(IntegrationStatus::Connecting.as_str(), "connecting");
        assert_eq!(IntegrationStatus::Disconnected.as_str(), "disconnected");
        assert_eq!(IntegrationStatus::Connected.as_str(), "connected");
    }

    #[test]
    fn cloud_status_as_str_all() {
        assert_eq!(CloudStatus::NoIntegration.as_str(), "no_integration");
        assert_eq!(CloudStatus::NotObserved.as_str(), "not_observed");
        assert_eq!(CloudStatus::Syncing.as_str(), "syncing");
        assert_eq!(CloudStatus::Synced.as_str(), "synced");
        assert_eq!(CloudStatus::Error.as_str(), "error");
    }
}
