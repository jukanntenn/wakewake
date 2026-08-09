//! Agent 内存状态（device-sync-v3 §10.1）。
//!
//! agent 持有最小状态集：`applied_version`（投影单调守卫）+ `projection`（最新快照）。
//! 收到 state 事件：teardown 旧状态 → apply 新状态 → 更新 applied_version。
//! INIT→APPLIED：收到任意首条快照即转 APPLIED（不判 version，§10.2），之后才启用 I2 单调守卫。
//!
//! 用 `std::sync::RwLock（非` `tokio::sync::RwLock`）：SSE 事件回调是同步的。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wakewake_protocol::{DeviceData, IntegrationData, StateSnapshot};

/// Agent 内存状态（devices + integrations + applied_version + aid）。teardown-then-rebuild 规约（§10.2）。
#[derive(Debug, Default)]
pub struct AgentState {
    pub devices: HashMap<String, DeviceData>,
    pub integrations: Vec<IntegrationData>,
    /// 投影单调水位（I2 守卫：仅在 version > applied_version 时应用）。
    /// 0 = INIT（未应用任何快照）。
    pub applied_version: u64,
    /// 当前 agent 的 aid（首条快照缓存，k4 派生输入，§6）。
    pub aid: Option<String>,
}

pub type SharedState = Arc<RwLock<AgentState>>;

/// 应用快照的结果（供调用方决定是否上报 ack + notify 对账）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// INIT→APPLIED 或 version > applied：应用了快照（上报 ack + notify 对账）。
    Applied { version: u64 },
    /// version ≤ applied：幂等丢弃（仍上报 ack，§10.2）。
    IdempotentDropped { version: u64 },
}

impl AgentState {
    #[must_use]
    pub fn new_shared() -> SharedState {
        Arc::new(RwLock::new(Self::default()))
    }

    /// 应用快照（teardown-then-rebuild）。
    ///
    /// §10.2：INIT 态（applied_version == 0 且从未应用）不判 version，任意首条快照转 APPLIED；
    /// APPLIED 态启用 I2 守卫（version > applied 才应用）。
    /// 返回 ApplyOutcome 供调用方决定上报/notify。
    pub fn apply_snapshot(&mut self, snapshot: StateSnapshot) -> ApplyOutcome {
        let version = snapshot.version;
        let was_init = self.aid.is_none();
        // 缓存 aid（每条 state 都带，§8.6）
        self.aid = Some(snapshot.aid);

        if !was_init && version <= self.applied_version {
            // APPLIED 态 + version ≤ applied → 幂等丢弃（§10.2 仍上报 ack）
            return ApplyOutcome::IdempotentDropped { version };
        }

        // 应用（teardown-then-rebuild）
        self.devices.clear();
        for d in snapshot.devices {
            self.devices.insert(d.did.clone(), d);
        }
        self.integrations = snapshot.integrations;
        self.applied_version = version;
        ApplyOutcome::Applied { version }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(version: u64, aid: &str, devices: Vec<(&str, &str)>) -> StateSnapshot {
        StateSnapshot {
            version,
            aid: aid.into(),
            devices: devices
                .into_iter()
                .map(|(did, name)| DeviceData {
                    did: did.into(),
                    name: name.into(),
                    mac_encrypted: "ct".into(),
                    description: None,
                })
                .collect(),
            integrations: vec![],
        }
    }

    #[test]
    fn init_state_applies_first_snapshot_without_version_guard() {
        // §10.2：INIT 态首条快照（哪怕 version=0）必须转 APPLIED
        let mut state = AgentState::default();
        let outcome = state.apply_snapshot(snap(0, "aid123", vec![("d1", "NAS")]));
        assert!(matches!(outcome, ApplyOutcome::Applied { version: 0 }));
        assert_eq!(state.applied_version, 0);
        assert_eq!(state.aid.as_deref(), Some("aid123"));
        assert_eq!(state.devices.len(), 1);
    }

    #[test]
    fn applied_state_applies_higher_version() {
        let mut state = AgentState::default();
        state.apply_snapshot(snap(5, "aid", vec![]));
        let outcome = state.apply_snapshot(snap(8, "aid", vec![("d1", "PC")]));
        assert!(matches!(outcome, ApplyOutcome::Applied { version: 8 }));
        assert_eq!(state.applied_version, 8);
        assert_eq!(state.devices.len(), 1);
    }

    #[test]
    fn applied_state_idempotent_drops_equal_or_lower_version() {
        let mut state = AgentState::default();
        state.apply_snapshot(snap(5, "aid", vec![("d1", "PC")]));
        // version == applied → 丢弃
        let outcome = state.apply_snapshot(snap(5, "aid", vec![]));
        assert!(matches!(
            outcome,
            ApplyOutcome::IdempotentDropped { version: 5 }
        ));
        // devices 不变（未被空快照覆盖）
        assert_eq!(state.devices.len(), 1);
        // version < applied → 丢弃
        let outcome2 = state.apply_snapshot(snap(3, "aid", vec![]));
        assert!(matches!(
            outcome2,
            ApplyOutcome::IdempotentDropped { version: 3 }
        ));
    }

    #[test]
    fn teardown_then_apply_replaces_state() {
        let mut state = AgentState::default();
        state.apply_snapshot(snap(1, "aid1", vec![("d1", "NAS")]));
        // 第二次 apply（更高 version）：teardown 旧 devices
        state.apply_snapshot(snap(2, "aid1", vec![("d2", "PC")]));
        assert_eq!(state.devices.len(), 1);
        assert!(state.devices.contains_key("d2"));
        assert!(!state.devices.contains_key("d1"));
    }
}
