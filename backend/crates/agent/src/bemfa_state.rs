//! Bemfa（巴法云）协调器：device-sync-v3 §10 Reconciler（Notify 调度器 + 锁外 IO reconcile）。
//!
//! 核心设计（详见 specs/backend/device-sync-v3.md §10）：
//! - **agent 端云端记忆 = 零**（每轮从 allTopic 现读，I3）。
//! - **Notify 调度器**（§10.3）：`timeout(interval, notify.notified())`，all_settled 切 5min/10s 自适应。
//! - **reconcile 顺序**（§10.7）：① allTopic 现读（失败整轮放弃 I3）② 算 observations
//!   ③ POST /sync（applied_version + integration 块；报告失败不修复 I5）④ create/modifyName/deleteTopic
//!   （仅当 current_version==V_snap 才 delete，I6 守卫）。
//! - **集成运行时两态**（§10.6）：INACTIVE/ACTIVE，config 指纹变 → 重启 MQTT loop。
//! - **watch 通道**（§10.5）：MQTT loop 增量订阅。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rsa::RsaPrivateKey;
use tokio::sync::{Mutex, Notify, watch};
use wakewake_protocol::{IntegrationData, Observation, RepairError, SyncIntegration, SyncRequest};

use crate::bemfa::{self, V2Credentials};
use crate::config::WolSettings;
use crate::reporter;
use crate::state::SharedState;

/// 自适应对账 tick（§10.3）：全一致 5min，有差异 10s。
const TICK_FAST: std::time::Duration = std::time::Duration::from_secs(10);
const TICK_SLOW: std::time::Duration = std::time::Duration::from_mins(5);

/// 孤儿删除开关（§2.1 / §10.9 / §12.4）。
/// 已实测确认巴法 allTopic 接口一次性全量返回（无分页/cursor，§12.4 前提已满足），
/// 故默认开启。I6 版本守卫（current_version == applied_version）保证不基于陈旧副本误删。
const ORPHAN_DELETE_ENABLED: bool = true;

/// 解密后的巴法运行时配置（uid/可选 v2 凭证）——config 指纹比较锚点（§10.6）。
///
/// 比较**解密后明文**而非 §10.6 字面建议的「对存储密文求 hash」：RSA-OAEP 非确定性，
/// server 重写 config 时即使明文不变密文也会变，比明文更鲁棒（避免误判配置变化重启 MQTT loop）。
/// 解密失败的集成无法算指纹——那直接落到 `report_decrypt_error` 早退，不会进 ACTIVE。
#[derive(Clone, PartialEq, Eq)]
struct RuntimeConfig {
    uid: String,
    v2: Option<V2Credentials>,
}

/// 解密失败的集成错误（§13.5.1）。
struct DecryptError(String);

/// 从 integration config 解密 uid（失败返错误）。
fn decrypt_uid(integration: &IntegrationData, pk: &RsaPrivateKey) -> Result<String, DecryptError> {
    let uid_enc = integration
        .config
        .get("uid")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    crate::crypto::decrypt(pk, uid_enc)
        .map_err(|e| DecryptError(format!("uid decrypt failed: {e}")))
}

/// uid 明文格式校验（§13.5.1：32 hex 或 45 [A-Za-z0-9_-]）。
fn is_valid_uid(uid: &str) -> bool {
    let is_32_hex = uid.len() == 32 && uid.chars().all(|c| c.is_ascii_hexdigit());
    let is_45_char = uid.len() == 45
        && uid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    is_32_hex || is_45_char
}

/// 从 integration config 解密 v2 凭证（成对；任一缺失/失败 → None 走 v1）。
fn decrypt_v2(integration: &IntegrationData, pk: &RsaPrivateKey) -> Option<V2Credentials> {
    let sid_enc = integration.config.get("secret_id")?.as_str()?;
    let skey_enc = integration.config.get("secret_key")?.as_str()?;
    let sid = crate::crypto::decrypt(pk, sid_enc).ok()?;
    let skey = crate::crypto::decrypt(pk, skey_enc).ok()?;
    Some(V2Credentials {
        secret_id: sid,
        secret_key: skey,
    })
}

/// 协调器内部状态（受 mutex 保护）。
struct CoordinatorInner {
    /// 当前 MQTT loop 的 cancel handle（None = 未连）。
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    /// topic 集合 watch 发送端。
    topics_tx: Option<watch::Sender<Vec<String>>>,
    /// 当前运行的配置指纹（None = 未连）。
    current: Option<RuntimeConfig>,
    /// MQTT 实际连接状态镜像（与 run_mqtt_loop 共享）。
    mqtt_connected: Arc<AtomicBool>,
    /// 上一轮修复错误，本轮带上报 server（§10.7：本轮修复结果下轮报）。
    /// 每轮开头取出清空，结尾由本轮新错误填充；集成都没了时 teardown 清空。
    pending_repair_errors: Vec<RepairError>,
}

/// Bemfa 协调器（agent 内单例，由 main 持有）。
#[derive(Clone)]
pub struct BemfaCoordinator {
    inner: Arc<Mutex<CoordinatorInner>>,
    /// Notify（对账调度器触发源：投影应用 / MQTT 重连）。
    notify: Arc<Notify>,
}

/// reconcile 调度器依赖（注入，避免 Reconciler 持有过多字段）。
pub struct ReconcilerDeps {
    pub state: SharedState,
    pub private_key: RsaPrivateKey,
    pub wol_settings: WolSettings,
    pub http_client: reqwest::Client,
    pub server_url: String,
    pub pairing_code: String,
}

impl BemfaCoordinator {
    /// 创建协调器。初始状态：未连接。
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CoordinatorInner {
                cancel: None,
                topics_tx: None,
                current: None,
                mqtt_connected: Arc::new(AtomicBool::new(false)),
                pending_repair_errors: Vec::new(),
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    /// 投影应用 / MQTT 重连后调用：唤醒对账调度器。
    pub fn notify_one(&self) {
        self.notify.notify_one();
    }

    /// 启动对账调度器主循环（§10.3）。应在 main spawn 为独立 task，阻塞到进程退出。
    pub async fn run_scheduler(self: Arc<Self>, deps: ReconcilerDeps) {
        let mut interval = TICK_FAST; // 初始快档，确保首对账尽快发生
        loop {
            // 门铃（notify）或超时（tick），谁先到就触发
            let _ = tokio::time::timeout(interval, self.notify.notified()).await;
            let outcome = self.reconcile(&deps).await;
            interval = if outcome.all_settled {
                TICK_SLOW
            } else {
                TICK_FAST
            };
        }
    }

    /// 单轮 reconcile 入口（驱动一轮对账）。生产由 `run_scheduler` 循环调用，
    /// 集成测试可直接调单轮以断言跨轮状态（§10.7 repair_errors 携带等）。
    pub async fn reconcile_once(&self, deps: &ReconcilerDeps) -> bool {
        self.reconcile(deps).await.all_settled
    }

    /// 当前挂起的修复错误（上一轮收集、待本轮上报 server，§10.7）。
    pub async fn pending_repair_errors(&self) -> Vec<RepairError> {
        self.inner.lock().await.pending_repair_errors.clone()
    }

    /// reconcile 一轮（§10.4 锁外 IO + §10.7 顺序）。
    ///
    /// 返回 all_settled 谓词（§10.3：无解密错 ∧ allTopic 成功 ∧ 无 create/modify ∧ repair_errors 空）。
    #[allow(clippy::too_many_lines)]
    async fn reconcile(&self, deps: &ReconcilerDeps) -> ReconcileOutcome {
        // 1. 短暂持锁，clone 快照副本 + 读 aid/applied_version（§10.4 不跨 await 持锁）
        let (integration, devices, aid_simple, v_snap) = {
            let s = deps.state.read().expect("state lock");
            let integration = s
                .integrations
                .iter()
                .find(|i| i.provider == "bemfa")
                .cloned();
            let devices: Vec<(String, String)> = s
                .devices
                .values()
                .map(|d| (d.did.clone(), d.name.clone()))
                .collect();
            (integration, devices, s.aid.clone(), s.applied_version)
        };
        // 取出上轮挂起的修复错误，本轮带上报 server（§10.7：本轮修复结果下轮报）。
        // std::mem::take 清空挂起，避免失败错误被无限重报。
        let pending_repair_errors =
            std::mem::take(&mut self.inner.lock().await.pending_repair_errors);

        let Some(integration) = integration else {
            // 集成消失 → INACTIVE（断 MQTT、保留 topic）
            self.teardown_loop().await;
            return ReconcileOutcome { all_settled: true };
        };
        if !integration.enabled {
            // 集成禁用 → INACTIVE（断 MQTT、保留 topic，§11）
            self.teardown_loop().await;
            return ReconcileOutcome { all_settled: true };
        }
        let Some(aid_simple) = aid_simple else {
            // aid 未就绪（首条快照未到），跳过本轮
            return ReconcileOutcome { all_settled: false };
        };

        // 2. 解密凭证（§13.5.1，失败上报 integration.error，本轮放弃）
        let uid = match decrypt_uid(&integration, &deps.private_key) {
            Ok(u) => u,
            Err(DecryptError(msg)) => {
                tracing::warn!(error = %msg, "bemfa: uid decrypt failed");
                self.report_decrypt_error(deps, &integration, &msg, v_snap)
                    .await;
                return ReconcileOutcome { all_settled: false };
            },
        };
        if !is_valid_uid(&uid) {
            let msg = "uid 格式错误：需 32 位十六进制或 45 位字符";
            tracing::warn!(uid_len = uid.len(), "bemfa: uid format invalid");
            self.report_decrypt_error(deps, &integration, msg, v_snap)
                .await;
            return ReconcileOutcome { all_settled: false };
        }
        let v2 = decrypt_v2(&integration, &deps.private_key);
        let new_config = RuntimeConfig {
            uid: uid.clone(),
            v2: v2.clone(),
        };

        // 3. config 变更或首次 → 重启 MQTT loop（§10.6 配置指纹）
        let config_changed = {
            let inner = self.inner.lock().await;
            inner.current.as_ref() != Some(&new_config)
        };
        if config_changed {
            self.teardown_loop().await;
            // 起新 loop（用新 config 的 topic 初始集）
            let initial_topics: Vec<String> = devices
                .iter()
                .map(|(did, _)| bemfa::device_topic(&aid_simple, did))
                .collect();
            let (topics_tx, topics_rx) = watch::channel(initial_topics.clone());
            let mqtt_connected = {
                let mut inner = self.inner.lock().await;
                inner.mqtt_connected.store(false, Ordering::Relaxed);
                inner.current = Some(new_config.clone());
                inner.mqtt_connected.clone()
            };
            let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
            {
                let mut inner = self.inner.lock().await;
                inner.cancel = Some(cancel_tx);
                inner.topics_tx = Some(topics_tx);
            }
            let opts = bemfa::MqttLoopOptions {
                uid: uid.clone(),
                aid_simple: aid_simple.clone(),
                initial_topics,
                mqtt_connected,
                topics_rx,
            };
            let notify = self.notify.clone();
            let deps_clone = DepsClone {
                state: deps.state.clone(),
                pk: deps.private_key.clone(),
                wol: deps.wol_settings.clone(),
                http: deps.http_client.clone(),
                surl: deps.server_url.clone(),
                code: deps.pairing_code.clone(),
            };
            tokio::spawn(async move {
                bemfa::run_mqtt_loop(
                    opts,
                    move |did| {
                        let d = deps_clone.clone();
                        tokio::spawn(async move {
                            bemfa::trigger_wol(
                                &d.state, &d.pk, &d.wol, &did, &d.http, &d.surl, &d.code,
                            )
                            .await;
                        });
                    },
                    notify,
                    cancel_rx,
                )
                .await;
            });
        }

        // 4. allTopic 现读（§10.7 ①，I3：失败整轮放弃）
        let cloud_topics = match bemfa::list_all_topics_detail(&deps.http_client, &uid).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, "bemfa: allTopic failed, aborting round (I3)");
                return ReconcileOutcome { all_settled: false };
            },
        };

        // 5. 算 observations（§10.7 ②）：每设备 topic 纯函数算 + 查 allTopic 得 observed_name
        let cloud_by_topic: HashMap<String, &bemfa::TopicItem> =
            cloud_topics.iter().map(|t| (t.topic.clone(), t)).collect();
        let mut observations = Vec::with_capacity(devices.len());
        let mut to_create = Vec::new();
        let mut to_modify = Vec::new();
        for (did, name) in &devices {
            let topic = bemfa::device_topic(&aid_simple, did);
            if let Some(item) = cloud_by_topic.get(&topic) {
                let observed_name = item.name.clone();
                observations.push(Observation {
                    did: did.clone(),
                    observed_name: observed_name.clone(),
                });
                // 云昵称≠期望名 → modifyName（幂等可逆，不守卫）
                if observed_name.as_deref() != Some(name.as_str()) {
                    to_modify.push((topic.clone(), name.clone()));
                }
            } else {
                // topic 不存在 → 观测 ∅ + create
                observations.push(Observation {
                    did: did.clone(),
                    observed_name: None,
                });
                to_create.push((topic.clone(), name.clone()));
            }
        }

        // 6. POST /sync（§10.7 ③：报告失败不修复 I5；applied_version=V_snap 本轮基准）
        let sync_req = SyncRequest {
            applied_version: Some(v_snap),
            integration: Some(SyncIntegration {
                provider: "bemfa".into(),
                enabled: integration.enabled,
                mqtt_connected: {
                    let inner = self.inner.lock().await;
                    inner.mqtt_connected.load(Ordering::Relaxed)
                },
                error: None,
                observations,
                // 上轮修复错误本轮报（§10.7）；本轮新错误在步骤 7 收集后挂起，下轮报。
                repair_errors: pending_repair_errors,
            }),
        };
        let sync_resp = match reporter::sync(
            &deps.http_client,
            &deps.server_url,
            &deps.pairing_code,
            sync_req,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "bemfa: sync report failed, not repairing (I5)");
                return ReconcileOutcome { all_settled: false };
            },
        };
        // I6 守卫：current_version == V_snap 才 delete（§10.9）
        let current_version = i64::try_from(sync_resp.current_version).unwrap_or(-1);
        let guard_passed = current_version == i64::try_from(v_snap).unwrap_or(-1);

        // 7. 修复（§10.7 ④）：create / modifyName（不守卫）+ delete（守卫）
        let mut repair_errors: Vec<RepairError> = Vec::new();
        for (topic, name) in &to_create {
            let params = bemfa::CreateTopicParams {
                uid: &uid,
                topic,
                name: Some(name.as_str()),
                v2: v2.as_ref(),
            };
            if let Err(e) = bemfa::create_topic(&deps.http_client, &params).await {
                tracing::warn!(topic = %topic, error = %e, "createTopic failed");
                // 提取 did 加入 repair_errors（用 topic 反解）
                if let Some(did) = bemfa::parse_did_from_topic(&aid_simple, topic) {
                    repair_errors.push(RepairError { did, error: e });
                }
            }
        }
        for (topic, name) in &to_modify {
            if let Err(e) = bemfa::modify_name(&deps.http_client, &uid, topic, name).await {
                tracing::warn!(topic = %topic, error = %e, "modifyName failed");
                if let Some(did) = bemfa::parse_did_from_topic(&aid_simple, topic) {
                    repair_errors.push(RepairError {
                        did,
                        error: format!("modifyName: {e}"),
                    });
                }
            }
        }
        // 孤儿删除（§10.9 + §12.4：默认 OFF）
        if ORPHAN_DELETE_ENABLED && guard_passed {
            let owned_topic_set: std::collections::HashSet<String> = devices
                .iter()
                .map(|(did, _)| bemfa::device_topic(&aid_simple, did))
                .collect();
            for item in &cloud_topics {
                if bemfa::is_owned_topic(&aid_simple, &item.topic)
                    && !owned_topic_set.contains(&item.topic)
                {
                    if let Err(e) = bemfa::delete_topic(&deps.http_client, &uid, &item.topic).await
                    {
                        tracing::warn!(topic = %item.topic, error = %e, "orphan delete failed");
                    }
                }
            }
        } else if !ORPHAN_DELETE_ENABLED && !guard_passed {
            tracing::debug!(
                current_version,
                v_snap,
                "orphan delete skipped (I6 version guard)"
            );
        }

        // 8. 更新 watch topic 集合（§10.5 增量订阅依据）
        let new_topic_set: Vec<String> = devices
            .iter()
            .map(|(did, _)| bemfa::device_topic(&aid_simple, did))
            .collect();
        if let Some(tx) = &self.inner.lock().await.topics_tx {
            tx.send(new_topic_set).ok();
        }

        // all_settled（§10.3）：无解密错 ∧ allTopic 成功 ∧ 无 create/modify ∧ repair_errors 空
        let all_settled = to_create.is_empty() && to_modify.is_empty() && repair_errors.is_empty();
        // 本轮修复错误挂起，下轮带上报 server（§10.7：本轮修复在报告之后）。
        if !repair_errors.is_empty() {
            self.inner.lock().await.pending_repair_errors = repair_errors;
        }
        ReconcileOutcome { all_settled }
    }

    /// 上报解密失败的 integration.error（§13.5.1）。
    async fn report_decrypt_error(
        &self,
        deps: &ReconcilerDeps,
        integration: &IntegrationData,
        msg: &str,
        v_snap: u64,
    ) {
        let req = SyncRequest {
            applied_version: Some(v_snap),
            integration: Some(SyncIntegration {
                provider: integration.provider.clone(),
                enabled: integration.enabled,
                mqtt_connected: false,
                error: Some(msg.to_string()),
                observations: vec![],
                repair_errors: vec![],
            }),
        };
        if let Err(e) =
            reporter::sync(&deps.http_client, &deps.server_url, &deps.pairing_code, req).await
        {
            tracing::warn!(error = %e, "bemfa: report decrypt error failed");
        }
    }

    /// 断开 MQTT loop（cancel + 清 current）。不删云 topic（§11.2）。
    async fn teardown_loop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(cancel) = inner.cancel.take() {
            let _ = cancel.send(());
        }
        inner.topics_tx = None;
        inner.mqtt_connected.store(false, Ordering::Relaxed);
        inner.current = None;
        // 集成消失/禁用，挂起的修复错误失去上报意义，清空。
        inner.pending_repair_errors.clear();
    }
}

impl Default for BemfaCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// reconcile 结果（all_settled 谓词，§10.3）。
struct ReconcileOutcome {
    all_settled: bool,
}

/// 传给 run_mqtt_loop on_wake 闭包的依赖快照（Clone 友好）。
#[derive(Clone)]
struct DepsClone {
    state: SharedState,
    pk: RsaPrivateKey,
    wol: WolSettings,
    http: reqwest::Client,
    surl: String,
    code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_uid_32_hex_accepted() {
        assert!(is_valid_uid("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"));
    }

    #[test]
    fn valid_uid_45_char_accepted() {
        let uid = "A".repeat(45);
        assert!(is_valid_uid(&uid));
    }

    #[test]
    fn invalid_uid_rejected() {
        assert!(!is_valid_uid(""));
        assert!(!is_valid_uid("too-short"));
        assert!(!is_valid_uid("z1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4")); // 非 hex
    }

    #[test]
    fn new_coordinator_starts_disconnected() {
        let coord = BemfaCoordinator::new();
        let inner = coord.inner.try_lock();
        assert!(inner.is_ok());
        let inner = inner.unwrap();
        assert!(inner.cancel.is_none());
        assert!(inner.current.is_none());
        assert!(!inner.mqtt_connected.load(Ordering::Relaxed));
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn orphan_delete_enabled_by_default() {
        // §12.4：已实测确认 allTopic 全量返回（无分页），前提满足 → 默认开启。
        // I6 版本守卫（current_version == applied_version）保证不误删。
        assert!(ORPHAN_DELETE_ENABLED);
    }
}
