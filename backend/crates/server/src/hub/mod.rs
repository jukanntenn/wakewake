//! SSE 命令派发（内存 Hub）。无 DB，纯内存（onion-architecture）。
//!
//! device-sync-v3 重构：投影通道用单调 `version` + 全量快照 + **server gap 重推**（§5）。
//! - `AgentConnection` 持 `last_pushed_version` / `last_acked_version`（§5.4.1）。
//! - 连接建立时两值初始化为握手快照 V（§5.4.1 关键初值，防重连后永久 syncing）。
//! - gap 重推每 5s tick：纯内存比较 `last_acked < last_pushed` 找落后 agent，
//!   每个落后 agent 在一个 tick 内**只读一次** DB 当前快照（§5.5）。
//!   不同 agent 因数据隔离（各自 device/integration 集）各自读自己的快照，
//!   不存在跨 agent 共享同一份快照的可能。
//! - command 通道只承载 `Wol`（`s_` state-ack 占位已废）。

pub mod command;

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use wakewake_protocol::StateSnapshot;

use crate::domain::command::{Command, CommandOutcome, CommandStatus};

pub use command::DispatcherError;

/// 单个 agent 的 SSE 连接态（§5.4.1）。
///
/// 纯内存，server 重启即丢失（agent 重连重建，握手快照自愈）。
#[derive(Debug)]
pub struct AgentConnection {
    /// SSE 推送通道（每 agent 一条）。
    pub tx: mpsc::Sender<SseEvent>,
    /// 最后**推送成功**的快照版本（推送成功后更新；失败保持旧值让 gap 重推，§5.4.1）。
    pub last_pushed_version: i64,
    /// 最后收到的 ack（来自 `POST /sync` 的 `applied_version`；`max` 语义更新）。
    pub last_acked_version: i64,
}

/// 经 SSE 下发给 agent 的事件。command 通道与 state 通道复用同一 mpsc。
///
/// device-sync-v3：`State` 携带单调 `version`（agent 据此 I2 单调应用）。
#[derive(Debug, Clone)]
pub enum SseEvent {
    /// SSE `event: command`（唤醒等命令）。
    Command(Command),
    /// SSE `event: state`——一份新的全量快照 + 单调版本。**不带 SSE `id:`**（§8.6，不实现 resume）。
    State {
        snapshot: StateSnapshot,
        version: i64,
    },
}

/// 命令分发端口（trait）。内存实现当前，未来 Redis 实现换类型参数（泛型零成本抽象）。
pub trait CommandDispatcher: Clone + Send + Sync + 'static {
    /// 派发命令给在线 agent。agent 离线返 Err（命令不进队列）。
    fn dispatch(&self, agent_id: i64, command: Command) -> Result<(), DispatcherError>;

    /// 推送全量 state 快照给在线 agent（投影通道，§5.4）。
    /// 成功（`tx.send().is_ok()`）才更新 `last_pushed_version = version`；失败保持旧值让 gap 重推。
    /// agent 离线（无连接）→ Err（agent 重连时 SSE 握手会推全量）。
    fn push_state(
        &self,
        agent_id: i64,
        snapshot: StateSnapshot,
        version: i64,
    ) -> Result<(), DispatcherError>;

    /// agent online 时注册连接（SSE handler 调用）。`initial_version` = 握手快照 V，
    /// 用于初始化 `last_pushed_version = last_acked_version = V`（§5.4.1 关键初值）。
    fn subscribe(&self, agent_id: i64, initial_version: i64) -> mpsc::Receiver<SseEvent>;

    /// agent 断开时注销（SSE 连接结束）。
    fn unsubscribe(&self, agent_id: i64);

    /// 记录 agent 上报的 ack 水位（`POST /sync` 的 `applied_version`，§5.4.1 `max` 语义）。
    fn record_ack(&self, agent_id: i64, applied_version: i64);

    /// 查命令状态（GET /commands/:id，内存资源，60s 过期后 None）。
    fn get_command(&self, command_id: &str) -> Option<Command>;

    /// complete 端点：agent 回报命令终态。
    fn complete(&self, command_id: &str, outcome: CommandOutcome) -> bool;

    /// 标记命令 expired（sweep task 用）。
    fn expire(&self, command_id: &str);

    /// agent 是否在线（SSE 连接在）。
    fn is_online(&self, agent_id: i64) -> bool;

    /// 当前在线 agent 连接数（admin stats 用，O(1) 读 dashmap len）。
    fn connected_count(&self) -> usize;

    /// 存储 offline 命令（agent 离线时 dispatch 失败，存入 hub 供 sweep 过期）。
    /// §8.4.1：agent 离线时命令 60s expired，写 wake(expired)。
    fn store_offline_command(&self, command: Command);

    /// 返回落后 agent 列表（gap 重推 tick 用，§5.5）：`last_acked_version < last_pushed_version`。
    /// 纯内存整数比较，不读 DB。
    fn lagged_agents(&self) -> Vec<(i64, mpsc::Sender<SseEvent>, i64)> {
        Vec::new()
    }
}

/// 内存 Hub：agents map（agent_id → AgentConnection）+ commands map（id → Command）。
#[derive(Clone, Default)]
pub struct Hub {
    /// `agent_id` → 连接态。Some = SSE 连接在（online）。
    agents: Arc<DashMap<i64, AgentConnection>>,
    /// `command_id` → Command（内存，60s TTL）。
    commands: Arc<DashMap<String, Command>>,
}

impl Hub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 启动一个后台 sweep task，清理 60s 过期命令（标记 expired + 写 wake(expired) 记录）。
    #[must_use]
    pub fn spawn_sweep(
        self,
        pool: sqlx::PgPool,
        wake_writer: tokio::sync::mpsc::Sender<crate::domain::wake::RecordWake>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let now = OffsetDateTime::now_utc();
                let ttl = Duration::from_secs(crate::domain::COMMAND_TTL_SECS);
                let mut to_expire = Vec::new();
                for entry in self.commands.iter() {
                    if entry.status == CommandStatus::Dispatched && now - entry.created_at > ttl {
                        to_expire.push(entry.id.clone());
                    }
                }
                for id in to_expire {
                    if let Some(mut cmd) = self.commands.get_mut(&id) {
                        cmd.status = CommandStatus::Expired;
                        cmd.completed_at = Some(OffsetDateTime::now_utc());
                        // 写 wake(expired) 记录（§8.4.1，command 通道仅 Wol）
                        let wakewake_protocol::CommandPayload::Wol { device } = &cmd.payload;
                        {
                            let user_id: i64 =
                                sqlx::query_scalar("SELECT user_id FROM agents WHERE id = $1")
                                    .bind(cmd.agent_id)
                                    .fetch_optional(&pool)
                                    .await
                                    .ok()
                                    .flatten()
                                    .unwrap_or(0);
                            if user_id > 0 {
                                let device_name: String =
                                    sqlx::query_scalar("SELECT name FROM devices WHERE did = $1")
                                        .bind(
                                            uuid::Uuid::parse_str(&device.did)
                                                .unwrap_or_else(|_| uuid::Uuid::nil()),
                                        )
                                        .fetch_optional(&pool)
                                        .await
                                        .ok()
                                        .flatten()
                                        .unwrap_or_else(|| device.did.clone());
                                let _ = wake_writer
                                    .try_send(crate::domain::wake::RecordWake {
                                        user_id,
                                        device_did: uuid::Uuid::parse_str(&device.did)
                                            .unwrap_or_else(|_| uuid::Uuid::nil()),
                                        device_name,
                                        r#type: crate::domain::wake::WakeType::Wol,
                                        status: crate::domain::wake::WakeStatus::Expired,
                                        message: Some("command expired (60s timeout)".into()),
                                    })
                                    .ok();
                            }
                        }
                    }
                }
            }
        })
    }

    /// 启动 5s gap 重推 task（§5.5）。每个 tick：
    /// 1. 纯内存比较找落后 agent（`last_acked < last_pushed`）
    /// 2. 对每个落后 agent 读一次 DB 当前快照（`snapshot_provider` 闭包）+ 重推
    ///
    /// `snapshot_provider`：按 agent_id 返回 `(StateSnapshot, version)`（调用方注入 build_for_agent，
    /// 避免本模块依赖 service 层）。按 agent 读是必要的——每个 agent 有独立的 device/integration 集，
    /// 无法跨 agent 共享同一份快照；同一 agent 在一个 tick 内只读一次（不会重复读）。
    #[must_use]
    pub fn spawn_gap_repush<F>(self, snapshot_provider: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(
                i64,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Option<(StateSnapshot, i64)>> + Send>,
            > + Send
            + Sync
            + 'static,
    {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                // 1. 纯内存比较找落后 agent（不读 DB）
                let lagged: Vec<i64> = self
                    .agents
                    .iter()
                    .filter(|conn| conn.last_acked_version < conn.last_pushed_version)
                    .map(|conn| *conn.key())
                    .collect();
                if lagged.is_empty() {
                    continue;
                }
                // 2. 对每个落后 agent：读当前快照（每 agent 一次）+ 重推
                for agent_id in lagged {
                    let Some((snapshot, version)) = snapshot_provider(agent_id).await else {
                        continue;
                    };
                    // 直接走 push_state 逻辑（更新 last_pushed_version）
                    let _ = self.push_state(agent_id, snapshot, version);
                }
            }
        })
    }
}

impl CommandDispatcher for Hub {
    fn dispatch(&self, agent_id: i64, command: Command) -> Result<(), DispatcherError> {
        let conn = self
            .agents
            .get(&agent_id)
            .ok_or(DispatcherError::AgentOffline)?;
        match conn.tx.try_send(SseEvent::Command(command.clone())) {
            Ok(()) => {
                drop(conn);
                let mut stored = command;
                stored.status = CommandStatus::Dispatched;
                self.commands.insert(stored.id.clone(), stored);
                Ok(())
            },
            Err(mpsc::error::TrySendError::Full(_)) => Err(DispatcherError::AgentOffline),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(DispatcherError::AgentOffline),
        }
    }

    fn push_state(
        &self,
        agent_id: i64,
        snapshot: StateSnapshot,
        version: i64,
    ) -> Result<(), DispatcherError> {
        let conn = self
            .agents
            .get(&agent_id)
            .ok_or(DispatcherError::AgentOffline)?;
        // §5.4.1：推送成功（is_ok）才更新 last_pushed_version；失败保持旧值让下次 gap 检测重推
        match conn.tx.try_send(SseEvent::State { snapshot, version }) {
            Ok(()) => {
                drop(conn);
                if let Some(mut conn) = self.agents.get_mut(&agent_id) {
                    conn.last_pushed_version = version;
                }
                Ok(())
            },
            Err(mpsc::error::TrySendError::Full(_)) => Err(DispatcherError::AgentOffline),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(DispatcherError::AgentOffline),
        }
    }

    fn subscribe(&self, agent_id: i64, initial_version: i64) -> mpsc::Receiver<SseEvent> {
        let (tx, rx) = mpsc::channel(32);
        // §5.4.1 关键初值：last_pushed_version = last_acked_version = V（握手快照版本）
        // 防重连后 last_acked(0) < last_pushed(V) 永真 → 永久 syncing。
        // 若 agent 真应用了 V_new > V，它会主动上报 ack 更新 last_acked；若幂等丢弃则维持 V=synced。
        self.agents.insert(
            agent_id,
            AgentConnection {
                tx,
                last_pushed_version: initial_version,
                last_acked_version: initial_version,
            },
        );
        rx
    }

    fn unsubscribe(&self, agent_id: i64) {
        self.agents.remove(&agent_id);
    }

    fn record_ack(&self, agent_id: i64, applied_version: i64) {
        if let Some(mut conn) = self.agents.get_mut(&agent_id) {
            // §5.4.1：max 语义（只增不减，防乱序 ack 回退）
            if applied_version > conn.last_acked_version {
                conn.last_acked_version = applied_version;
            }
        }
    }

    fn get_command(&self, command_id: &str) -> Option<Command> {
        self.commands.get(command_id).map(|c| c.clone())
    }

    fn complete(&self, command_id: &str, outcome: CommandOutcome) -> bool {
        if let Some(mut cmd) = self.commands.get_mut(command_id) {
            cmd.status = if outcome.success {
                CommandStatus::Completed
            } else {
                CommandStatus::Failed
            };
            cmd.result = Some(outcome);
            cmd.completed_at = Some(OffsetDateTime::now_utc());
            true
        } else {
            false
        }
    }

    fn expire(&self, command_id: &str) {
        if let Some(mut cmd) = self.commands.get_mut(command_id) {
            cmd.status = CommandStatus::Expired;
            cmd.completed_at = Some(OffsetDateTime::now_utc());
        }
    }

    fn is_online(&self, agent_id: i64) -> bool {
        self.agents.contains_key(&agent_id)
    }

    fn connected_count(&self) -> usize {
        self.agents.len()
    }

    fn store_offline_command(&self, mut command: Command) {
        command.status = CommandStatus::Dispatched;
        command.created_at = OffsetDateTime::now_utc();
        self.commands.insert(command.id.clone(), command);
    }

    fn lagged_agents(&self) -> Vec<(i64, mpsc::Sender<SseEvent>, i64)> {
        self.agents
            .iter()
            .filter(|conn| conn.last_acked_version < conn.last_pushed_version)
            .map(|conn| (*conn.key(), conn.tx.clone(), conn.last_pushed_version))
            .collect()
    }
}

/// 读 agent 的 `last_acked_version`（routes/devices 派生 projection_status 用）。
/// 不在线或无连接返回 None。
#[must_use]
pub fn acked_version(hub: &Hub, agent_id: i64) -> Option<i64> {
    hub.agents
        .get(&agent_id)
        .map(|conn| conn.last_acked_version)
}

/// 生成命令 ID：prefix + 8 Base62 字符。
#[must_use]
pub fn generate_command_id(prefix: &str) -> String {
    use getrandom::fill;
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut buf = [0u8; 8];
    // getrandom 0.3 API：fill(&mut [u8]) -> Result。
    let _ = fill(&mut buf);
    let mut id = String::with_capacity(prefix.len() + 8);
    id.push_str(prefix);
    for b in buf {
        id.push(ALPHABET[b as usize % ALPHABET.len()] as char);
    }
    id
}

/// 创建命令（构造 helper，供 service 调用）。
#[must_use]
pub fn new_command(
    id: String,
    agent_id: i64,
    payload: wakewake_protocol::CommandPayload,
) -> Command {
    Command {
        id,
        status: CommandStatus::Pending,
        payload,
        agent_id,
        created_at: OffsetDateTime::now_utc(),
        completed_at: None,
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wakewake_protocol::{CommandPayload, WolDevice};

    fn make_wol_command() -> Command {
        let id = generate_command_id(crate::domain::command::prefix::COMMAND);
        new_command(
            id,
            1,
            CommandPayload::Wol {
                device: WolDevice {
                    did: "h".into(),
                    mac_encrypted: "ct".into(),
                },
            },
        )
    }

    fn empty_snapshot(version: u64) -> StateSnapshot {
        StateSnapshot {
            version,
            aid: "550e8400e29b41d4a716446655440000".into(),
            devices: vec![],
            integrations: vec![],
        }
    }

    #[tokio::test]
    async fn dispatch_offline_agent_errors() {
        let hub = Hub::new();
        let cmd = make_wol_command();
        let err = hub.dispatch(999, cmd).unwrap_err();
        assert!(matches!(err, DispatcherError::AgentOffline));
    }

    #[tokio::test]
    async fn dispatch_online_agent_delivers_and_completes() {
        let hub = Hub::new();
        let mut rx = hub.subscribe(1, 0);
        let cmd = make_wol_command();
        let id = cmd.id.clone();
        hub.dispatch(1, cmd).unwrap();

        let received = rx.recv().await.unwrap();
        let SseEvent::Command(received_cmd) = received else {
            panic!("expected Command variant, got {received:?}");
        };
        assert_eq!(received_cmd.id, id);

        let ok = hub.complete(
            &id,
            CommandOutcome {
                success: true,
                message: "sent".into(),
            },
        );
        assert!(ok);
        let stored = hub.get_command(&id).unwrap();
        assert_eq!(stored.status, CommandStatus::Completed);
    }

    #[tokio::test]
    async fn subscribe_initializes_versions_to_handshake_v() {
        // §5.4.1 关键初值：subscribe(V) 后 last_pushed = last_acked = V
        let hub = Hub::new();
        let _rx = hub.subscribe(1, 42);
        let conn = hub.agents.get(&1).unwrap();
        assert_eq!(conn.last_pushed_version, 42);
        assert_eq!(conn.last_acked_version, 42);
    }

    #[tokio::test]
    async fn record_ack_uses_max_semantics() {
        // §5.4.1：max 语义，防乱序回退
        let hub = Hub::new();
        let _rx = hub.subscribe(1, 5);
        hub.record_ack(1, 8);
        assert_eq!(hub.agents.get(&1).unwrap().last_acked_version, 8);
        // 旧版本 ack 不回退
        hub.record_ack(1, 3);
        assert_eq!(hub.agents.get(&1).unwrap().last_acked_version, 8);
    }

    #[tokio::test]
    async fn record_ack_offline_agent_noop() {
        let hub = Hub::new();
        // 不在线 agent：record_ack 无副作用（不 panic）
        hub.record_ack(999, 8);
        assert!(!hub.is_online(999));
    }

    #[tokio::test]
    async fn push_state_updates_last_pushed_on_success() {
        let hub = Hub::new();
        let mut rx = hub.subscribe(1, 0);
        hub.push_state(1, empty_snapshot(5), 5).unwrap();
        let received = rx.recv().await.unwrap();
        let SseEvent::State { version, .. } = received else {
            panic!("expected State variant");
        };
        assert_eq!(version, 5);
        assert_eq!(hub.agents.get(&1).unwrap().last_pushed_version, 5);
    }

    #[tokio::test]
    async fn push_state_offline_errors() {
        // agent 离线：push 返 Err（重连握手兜底）
        let hub = Hub::new();
        let err = hub.push_state(999, empty_snapshot(5), 5).unwrap_err();
        assert!(matches!(err, DispatcherError::AgentOffline));
    }

    #[tokio::test]
    async fn lagged_agents_detects_acked_lt_pushed() {
        // §5.5 gap 检测：last_acked < last_pushed → 落后
        let hub = Hub::new();
        let _rx = hub.subscribe(1, 0);
        // 推 V=5（成功 → last_pushed=5，last_acked 仍 0）
        hub.push_state(1, empty_snapshot(5), 5).unwrap();
        let lagged = hub.lagged_agents();
        assert_eq!(lagged.len(), 1);
        // agent 上报 ack=5 后不再落后
        hub.record_ack(1, 5);
        assert!(hub.lagged_agents().is_empty());
    }

    #[test]
    fn command_id_format() {
        let id = generate_command_id(crate::domain::command::prefix::COMMAND);
        assert!(id.starts_with("c_"));
        assert_eq!(id.len(), 2 + 8);
    }

    #[test]
    fn unsubscribe_makes_agent_offline() {
        let hub = Hub::new();
        let _rx = hub.subscribe(1, 0);
        assert!(hub.is_online(1));
        hub.unsubscribe(1);
        assert!(!hub.is_online(1));
    }

    #[test]
    fn acked_version_helper_returns_none_when_offline() {
        let hub = Hub::new();
        assert!(acked_version(&hub, 999).is_none());
        let _rx = hub.subscribe(1, 7);
        assert_eq!(acked_version(&hub, 1), Some(7));
    }
}
