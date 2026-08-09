//! Bemfa（巴法云）MQTT 订阅 + topic 管理（device-sync-v3 §6 / §13）。
//!
//! 官方文档依据（.local/bemfa/）+ 生产实现参考（ha-xiaodu）：
//! - MQTT：broker `bemfa.com`，TLS 端口 9503；认证方式一（uid 作 client_id，无需账密）；
//!   支持 QoS 0/1、retain，**禁用 QoS 2**。
//! - topic 命名（§6.1）：`ww ‖ k4(aid) ‖ did(32hex) ‖ 006`，k4 = base36(u64::from_be_bytes(sha256(aid_simple)[..8]))[..4]。
//! - allTopic：openID = **原始 uid**（非 base64，对齐 ha-xiaodu 生产 + 巴法文档示例；偏离 §13.5 字面）。
//! - createTopic：v1 `/v1/createTopic`，v2 `/vs/web/v2/createTopic`（+secretID/secretKey）。40006 幂等成功。
//!
//! rumqttc `AsyncClient` 无 connect()，由 `eventloop.poll()` 隐式连接，auto-reconnect 内建。
//! 收到 "on" → 从 topic 反解 did → 找 device → RSA 解密 MAC → WoL → 上报 bemfa_wake。

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, Transport};
use serde::Deserialize;
use serde::de::Error as _;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

use crate::state::SharedState;
use crate::wol;

// ============================================================================
// 常量
// ============================================================================

/// 巴法云 MQTT broker（E2E 用 `WAKEWAKE_BEMFA__BROKER` 覆盖指向 mosquitto）。
pub const BEMFA_BROKER: &str = "bemfa.com";
/// TLS 加密端口（mqtt.md：9503 支持 TLS 1.2）。
pub const BEMFA_PORT: u16 = 9503;

/// 设备类型码：006 = 开关（§13.6）。消息只需 on/off，精确匹配 WoL 触发语义。
pub const DEVICE_TYPE_CODE: &str = "006";
/// 巴法 MQTT 协议类型（createTopic 的 type 字段）：1 = MQTT 协议设备。
pub const PROTOCOL_TYPE_MQTT: u8 = 1;
/// 巴法 "开" 消息（开关 006 的开指令，触发唤醒）。
pub const ON_MESSAGE: &str = "on";
/// wakewake 固定 topic 前缀（§6.1）。
pub const TOPIC_PREFIX: &str = "ww";
/// k4 长度（§6.1）。
const K4_LEN: usize = 4;
/// did 长度（UUID simple 32 hex）。
const DID_LEN: usize = 32;

/// 巴法 topic 管理 API。
/// E2E 用 `WAKEWAKE_BEMFA__API_BASE` 覆盖指向 mock。
const CREATE_TOPIC_URL_V1: &str = "https://pro.bemfa.com/v1/createTopic";
const CREATE_TOPIC_URL_V2: &str = "https://pro.bemfa.com/vs/web/v2/createTopic";
const DELETE_TOPIC_URL: &str = "https://pro.bemfa.com/v1/deleteTopic";
const MODIFY_NAME_URL: &str = "https://apis.bemfa.com/va/modifyName";
/// allTopic 列表 API。
/// **注意**：openID = 原始 uid（非 base64，对齐 ha-xiaodu 生产 + 巴法文档示例）。
const ALL_TOPIC_URL: &str = "http://apis.bemfa.com/vb/api/v2/allTopic";

const CODE_TOPIC_EXISTS: i64 = 40006;
const CODE_SUCCESS: i64 = 0;

// ============================================================================
// topic 命名（§6.1 纯函数，agent 与 server 必须字节级一致）
// ============================================================================

/// k4 派生：`base36(u64::from_be_bytes(sha256(aid_simple)[..8]))[..4]` 小写。
///
/// aid_simple = aid 的 `Uuid::simple()` 字符串（32 hex 无连字符）的 UTF-8 字节。
#[must_use]
pub fn compute_k4(aid_simple: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(aid_simple.as_bytes());
    let digest = hasher.finalize();
    let bytes: [u8; 8] = digest[..8].try_into().expect("sha256 first 8 bytes");
    let n = u64::from_be_bytes(bytes);
    // base36 小写：radix-36 转换，[0-9a-z]
    to_base36_lower(n)[..K4_LEN].to_string()
}

/// u64 → base36 小写字符串。
fn to_base36_lower(n: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    let mut n = n;
    while n > 0 {
        digits.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    digits.reverse();
    String::from_utf8(digits).expect("base36 ascii")
}

/// 构造 device 的 topic 名：`ww ‖ k4(aid) ‖ did ‖ 006`（§6.1，共 41 字符）。
#[must_use]
pub fn device_topic(aid_simple: &str, did: &str) -> String {
    let k4 = compute_k4(aid_simple);
    format!("{TOPIC_PREFIX}{k4}{did}{DEVICE_TYPE_CODE}")
}

/// 从 topic 反解 did（需 aid_simple 计算 k4 前缀做精确归属匹配）。
/// topic 结构：`ww ‖ k4(4) ‖ did(32hex) ‖ 006(3)` = 41 字符。
#[must_use]
pub fn parse_did_from_topic(aid_simple: &str, topic: &str) -> Option<String> {
    let k4 = compute_k4(aid_simple);
    let prefix = format!("{TOPIC_PREFIX}{k4}");
    let rest = topic.strip_prefix(&prefix)?;
    // rest 应为 did(32) + 006(3) = 35
    if rest.len() != DID_LEN + DEVICE_TYPE_CODE.len() {
        return None;
    }
    let (did, type_code) = rest.split_at(DID_LEN);
    if type_code != DEVICE_TYPE_CODE {
        return None;
    }
    Some(did.to_string())
}

/// 判断 topic 是否属于本部署（ww‖k4 前缀精确匹配，§6.3 跨部署隔离）。
#[must_use]
pub fn is_owned_topic(aid_simple: &str, topic: &str) -> bool {
    let k4 = compute_k4(aid_simple);
    let prefix = format!("{TOPIC_PREFIX}{k4}");
    topic.starts_with(&prefix)
}

// ============================================================================
// env 解析（E2E mock 缝隙）
// ============================================================================

fn api_base() -> Option<String> {
    std::env::var("WAKEWAKE_BEMFA__API_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn api_url(default: &str, path: &str) -> String {
    match api_base() {
        Some(base) => format!("{base}{path}"),
        None => default.to_string(),
    }
}

fn broker_addr() -> (String, u16) {
    let broker =
        std::env::var("WAKEWAKE_BEMFA__BROKER").unwrap_or_else(|_| BEMFA_BROKER.to_string());
    let port: u16 = std::env::var("WAKEWAKE_BEMFA__PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(BEMFA_PORT);
    (broker, port)
}

// ============================================================================
// HTTP API
// ============================================================================

/// 巴法 API 响应（HTTP 恒 200，成败看 body 内 code）。
#[derive(Debug, Deserialize)]
pub struct BemfaResponse {
    pub code: i64,
}

/// 捕获响应的状态码、Content-Type、原始文本，供反序列化失败时诊断。
/// 关键：先 `text()` 再 `serde_json::from_str`，分离「网络层错误」与「结构错误」。
async fn capture_response_for_diag(
    resp: reqwest::Response,
) -> (reqwest::StatusCode, Option<String>, String) {
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = resp.text().await.unwrap_or_default();
    (status, content_type, body)
}

/// 构造带诊断三元组的错误信息（status / content-type / body 片段）。
/// body 截断到 256 字符，避免大数据量打爆日志。allTopic/createTopic 的 body
/// 不含本系统密钥（uid 是巴法 openID，非密文），记录安全。
fn diag(
    api: &str,
    err: impl std::fmt::Display,
    status: reqwest::StatusCode,
    content_type: Option<&String>,
    body: &str,
) -> String {
    let head: String = body.chars().take(256).collect();
    format!("{api} parse failed: {err}; status={status}; ct={content_type:?}; body_head={head:?}")
}

/// allTopic 响应里的单个 topic 项。name 为巴法云控制台展示昵称。
#[derive(Debug, Clone, Deserialize)]
pub struct TopicItem {
    pub topic: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// allTopic 响应体。
///
/// 巴法云真实 API：外层 `data` 多态——成功时是对象 `{"data":[...]}`（topic 数组
/// 再嵌一层），失败时是空字符串 `""`。用 `Value` 承接任一形态，在调用点手动
/// 提取内层数组，避免外层形态变化导致整体反序列化失败（可观测性：把「整体形态
/// 错」与「某个 topic 字段错」分离）。实测样本见 tests/bemfa_api.rs 的契约探针。
#[derive(Debug, Deserialize)]
struct AllTopicResponse {
    code: i64,
    #[serde(default)]
    data: serde_json::Value,
}

/// v2 API 凭证（secretID/secretKey，需实名认证）。None = 走 v1。
#[derive(Clone, PartialEq, Eq)]
pub struct V2Credentials {
    pub secret_id: String,
    pub secret_key: String,
}

/// createTopic 参数：v1（无 v2 凭证）或 v2（有凭证），可选 name。
pub struct CreateTopicParams<'a> {
    pub uid: &'a str,
    pub topic: &'a str,
    pub name: Option<&'a str>,
    pub v2: Option<&'a V2Credentials>,
}

/// 按 v2 凭证是否存在选 createTopic URL。
#[must_use]
pub fn create_topic_url(v2: Option<&V2Credentials>) -> String {
    if v2.is_some() {
        api_url(CREATE_TOPIC_URL_V2, "/vs/web/v2/createTopic")
    } else {
        api_url(CREATE_TOPIC_URL_V1, "/v1/createTopic")
    }
}

/// 创建 topic（v1/v2 自动路由，幂等：40006 视作成功）。
pub async fn create_topic(
    client: &reqwest::Client,
    params: &CreateTopicParams<'_>,
) -> Result<(), String> {
    create_topic_at(client, &create_topic_url(params.v2), params).await
}

/// `create_topic` 的可测变体（url 可注入）。
pub async fn create_topic_at(
    client: &reqwest::Client,
    url: &str,
    params: &CreateTopicParams<'_>,
) -> Result<(), String> {
    let mut body = serde_json::json!({
        "uid": params.uid,
        "topic": params.topic,
        "type": PROTOCOL_TYPE_MQTT,
    });
    if let Some(name) = params.name {
        body["name"] = serde_json::Value::String(name.to_string());
    }
    if let Some(v2) = params.v2 {
        body["secretID"] = serde_json::Value::String(v2.secret_id.clone());
        body["secretKey"] = serde_json::Value::String(v2.secret_key.clone());
    }
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("createTopic request failed: {e}"))?;
    let bemfa = resp
        .json::<BemfaResponse>()
        .await
        .map_err(|e| format!("createTopic response parse failed: {e}"))?;
    if bemfa.code == CODE_SUCCESS || bemfa.code == CODE_TOPIC_EXISTS {
        Ok(())
    } else {
        Err(format!("createTopic failed: code {}", bemfa.code))
    }
}

/// 删除 topic（v1，deleteTopic 无 v2 版本）。
pub async fn delete_topic(client: &reqwest::Client, uid: &str, topic: &str) -> Result<(), String> {
    delete_topic_at(
        client,
        &api_url(DELETE_TOPIC_URL, "/v1/deleteTopic"),
        uid,
        topic,
    )
    .await
}

/// `delete_topic` 的可测变体。
pub async fn delete_topic_at(
    client: &reqwest::Client,
    url: &str,
    uid: &str,
    topic: &str,
) -> Result<(), String> {
    let body = serde_json::json!({
        "uid": uid,
        "topic": topic,
        "type": PROTOCOL_TYPE_MQTT,
    });
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("deleteTopic request failed: {e}"))?;
    let bemfa = resp
        .json::<BemfaResponse>()
        .await
        .map_err(|e| format!("deleteTopic response parse failed: {e}"))?;
    if bemfa.code == CODE_SUCCESS {
        Ok(())
    } else {
        Err(format!("deleteTopic failed: code {}", bemfa.code))
    }
}

/// 修改 topic 的巴法控制台展示名（device 改名时同步）。
pub async fn modify_name(
    client: &reqwest::Client,
    uid: &str,
    topic: &str,
    name: &str,
) -> Result<(), String> {
    modify_name_at(
        client,
        &api_url(MODIFY_NAME_URL, "/va/modifyName"),
        uid,
        topic,
        name,
    )
    .await
}

/// `modify_name` 的可测变体。
pub async fn modify_name_at(
    client: &reqwest::Client,
    url: &str,
    uid: &str,
    topic: &str,
    name: &str,
) -> Result<(), String> {
    let body = serde_json::json!({
        "uid": uid,
        "topic": topic,
        "type": PROTOCOL_TYPE_MQTT,
        "name": name,
    });
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("modifyName request failed: {e}"))?;
    let bemfa = resp
        .json::<BemfaResponse>()
        .await
        .map_err(|e| format!("modifyName response parse failed: {e}"))?;
    if bemfa.code == CODE_SUCCESS {
        Ok(())
    } else {
        Err(format!("modifyName failed: code {}", bemfa.code))
    }
}

/// 列出账号下所有 topic（含昵称）。对账用：每轮现读（I3）。
///
/// **openID = 原始 uid**（非 base64，对齐 ha-xiaodu 生产 `api_client.py:241` + 巴法文档示例）。
pub async fn list_all_topics_detail(
    client: &reqwest::Client,
    uid: &str,
) -> Result<Vec<TopicItem>, String> {
    list_all_topics_detail_at(client, &api_url(ALL_TOPIC_URL, "/vb/api/v2/allTopic"), uid).await
}

/// `list_all_topics_detail` 的可测变体。
pub async fn list_all_topics_detail_at(
    client: &reqwest::Client,
    url: &str,
    uid: &str,
) -> Result<Vec<TopicItem>, String> {
    // openID = 原始 uid（明文，对齐 ha-xiaodu 生产 + 巴法文档示例；偏离 §13.5 base64 断言）
    let resp = client
        .get(url)
        .query(&[("openID", uid), ("type", "1")])
        .send()
        .await
        .map_err(|e| format!("allTopic request failed: {e}"))?;
    // 先 text() 再 from_str：分离「网络层错误」与「结构错误」（层1 可观测性）。
    let (status, content_type, body) = capture_response_for_diag(resp).await;
    let parsed: AllTopicResponse = serde_json::from_str(&body)
        .map_err(|e| diag("allTopic", e, status, content_type.as_ref(), &body))?;
    if parsed.code != CODE_SUCCESS {
        return Err(format!("allTopic failed: code {}", parsed.code));
    }
    // 手动提取内层 data 数组。巴法成功响应：data = {"data": [topic...]}。
    let topics = parsed
        .data
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| {
            diag(
                "allTopic",
                serde_json::Error::custom("missing nested data array"),
                status,
                content_type.as_ref(),
                &body,
            )
        })?;
    // 内层数组项仍走强类型反序列化（捕获字段级错误，如 topic 缺失）。
    serde_json::from_value::<Vec<TopicItem>>(serde_json::Value::Array(topics.clone())).map_err(
        |e| {
            diag(
                "allTopic (topic item)",
                e,
                status,
                content_type.as_ref(),
                &body,
            )
        },
    )
}

// ============================================================================
// MQTT loop（§10.5 watch 通道增量订阅）
// ============================================================================

/// MQTT loop 启动参数（由 Reconciler 组装）。cancel 单独作为 `run_mqtt_loop` 参数传递。
pub struct MqttLoopOptions {
    pub uid: String,
    pub aid_simple: String,
    /// 当前应订阅的 topic 集合（watch 初始值 = 投影中所有设备的 topic）。
    pub initial_topics: Vec<String>,
    /// MQTT 连接状态镜像（ConnAck/断线更新）。
    pub mqtt_connected: Arc<std::sync::atomic::AtomicBool>,
    /// topic 集合 watch 接收端（设备增删时增量 subscribe/unsubscribe）。
    pub topics_rx: watch::Receiver<Vec<String>>,
}

/// 启动 Bemfa MQTT 订阅循环（§10.5）。
///
/// - 认证：恒走方式一（uid 作 client_id，无账密）。
/// - **订阅时机**：每次 ConnAck success 后全量重订阅当前 topic 集合（watch 最新值）。
///   clean_session=true（rumqttc 默认）下 broker 每次连接清空订阅，故初始连接与重连
///   走同一路径——保证断线重连后订阅必然恢复（旧实现仅 loop 启动时订阅一次，重连即丢）。
/// - 运行时增量：select! 监听 watch.changed()，对设备增删做 subscribe/unsubscribe diff。
/// - eventloop.poll() 的 `Err` **不退出 loop**（rumqttc 内建重连）；ConnAck/断线 notify 调度器。
/// - 收到 "on" → 从 topic 反解 did → 调 `on_wake(did)`。
///
/// 此函数阻塞直到 cancel。
pub async fn run_mqtt_loop<F>(
    opts: MqttLoopOptions,
    on_wake: F,
    notify: Arc<tokio::sync::Notify>,
    mut cancel: tokio::sync::oneshot::Receiver<()>,
) where
    F: Fn(String) + Send + Sync + 'static,
{
    let (broker, port) = broker_addr();
    let mut mqttoptions = MqttOptions::new(opts.uid.clone(), broker, port);
    mqttoptions.set_keep_alive(Duration::from_secs(30));
    // 9503 TLS：用系统默认 CA（巴法公开证书）。非 9503（如 mosquitto 1883）走明文。
    if port == BEMFA_PORT {
        mqttoptions.set_transport(Transport::tls_with_default_config());
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let mut topics_rx = opts.topics_rx;
    let aid_simple = opts.aid_simple.clone();
    let on_wake = Arc::new(on_wake);
    let mqtt_connected = opts.mqtt_connected.clone();
    // 跟踪已订阅集合（增量 diff 基线）。初始化为空——首次订阅由 ConnAck success 分支
    // 统一执行（初始连接与重连走同一路径）。clean_session=true 下 broker 每次连接清空订阅，
    // 故每次 ConnAck success 都必须全量重订阅当前 topic 集合（见 ConnAck 分支）。
    let mut subscribed: std::collections::HashSet<String> = std::collections::HashSet::new();
    // 本地连接就绪标志。changed() 仅在 connected 时发 subscribe/unsubscribe，
    // 否则只更新 subscribed 目标集——避免连接建立前入队的 subscribe 与 ConnAck 后的
    // 全量重订阅叠加，引发 rumqttc 包解析时序错乱（Malformed packet）。
    // 下次 ConnAck success 会用 topics_rx 最新值全量重订阅，故连接前遗漏的订阅必然补齐。
    let mut connected = false;

    loop {
        tokio::select! {
            // cancel 优先：换 config 时终止旧 loop
            _ = &mut cancel => {
                tracing::info!("bemfa MQTT loop cancelled");
                set_mqtt_connected(&mqtt_connected, false);
                return;
            }
            // topic 集合变更（增量 subscribe/unsubscribe）
            result = topics_rx.changed() => {
                if result.is_ok() {
                    let new_set: std::collections::HashSet<String> =
                        topics_rx.borrow().clone().into_iter().collect();
                    if connected {
                        // 连接就绪：增量 diff 真实订阅/退订。
                        // 先收集差集（避免在 difference 迭代中 mutate subscribed），再执行 + 更新。
                        let to_subscribe: Vec<String> =
                            new_set.difference(&subscribed).cloned().collect();
                        let to_unsubscribe: Vec<String> =
                            subscribed.difference(&new_set).cloned().collect();
                        for t in &to_subscribe {
                            match client.subscribe(t, QoS::AtLeastOnce).await {
                                Ok(()) => tracing::info!(topic = %t, "MQTT subscribed (delta)"),
                                Err(e) => tracing::warn!(topic = %t, error = %e, "MQTT subscribe failed (delta)"),
                            }
                        }
                        for t in &to_unsubscribe {
                            match client.unsubscribe(t).await {
                                Ok(()) => tracing::info!(topic = %t, "MQTT unsubscribed (delta)"),
                                Err(e) => tracing::warn!(topic = %t, error = %e, "MQTT unsubscribe failed"),
                            }
                        }
                    }
                    // 连接未就绪时只更新目标集，不发 subscribe（避免连接前入队与 ConnAck
                    // 全量重订阅叠加引发 Malformed packet）。下次 ConnAck 会全量重订阅最新集。
                    subscribed = new_set;
                }
                // sender drop（loop 即将被 cancel/替换）→ changed() Err，忽略
            }
            result = eventloop.poll() => {
                match result {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        let payload = String::from_utf8_lossy(&p.payload);
                        let topic = p.topic.clone();
                        tracing::debug!(topic = %topic, payload = %payload, "MQTT message");
                        if payload.trim() == ON_MESSAGE {
                            if let Some(did) = parse_did_from_topic(&aid_simple, &topic) {
                                tracing::info!(%did, "bemfa wake triggered");
                                (*on_wake)(did);
                            } else {
                                tracing::warn!(%topic, "bemfa: could not parse did from topic");
                            }
                        }
                    }
                    Ok(Event::Incoming(Incoming::ConnAck(ack))) => {
                        use rumqttc::ConnectReturnCode;
                        match ack.code {
                            ConnectReturnCode::Success => {
                                tracing::info!("bemfa MQTT connected (ConnAck success)");
                                set_mqtt_connected(&mqtt_connected, true);
                                connected = true;
                                // clean_session=true：broker 每次连接清空订阅。
                                // 初始连接与重连走同一路径——ConnAck success 后全量重订阅
                                // 当前 topic 集合（取 watch 最新值，含运行中增删的设备）。
                                // clone 后立即 drop borrow，不跨 await。
                                let current: Vec<String> = topics_rx.borrow().clone();
                                for t in &current {
                                    match client.subscribe(t, QoS::AtLeastOnce).await {
                                        Ok(()) => tracing::info!(topic = %t, "MQTT subscribed"),
                                        Err(e) => tracing::warn!(topic = %t, error = %e, "MQTT subscribe failed"),
                                    }
                                }
                                subscribed = current.iter().cloned().collect();
                            }
                            code => {
                                tracing::warn!(?code, "bemfa MQTT ConnAck refused");
                                set_mqtt_connected(&mqtt_connected, false);
                                connected = false;
                            }
                        }
                        // §10.5：ConnAck 成败都 notify 调度器（让 mqtt_connected 变化及时上报）
                        notify.notify_one();
                    }
                    Ok(_) => {}
                    Err(e) => {
                        // §10.5：Err 不退出 loop（rumqttc 内建重连）
                        tracing::warn!(error = %e, "MQTT connection error, auto-reconnect");
                        set_mqtt_connected(&mqtt_connected, false);
                        connected = false;
                        notify.notify_one();
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }
}

fn set_mqtt_connected(flag: &std::sync::Arc<std::sync::atomic::AtomicBool>, value: bool) {
    flag.store(value, std::sync::atomic::Ordering::Relaxed);
}

/// MQTT "on" 触发唤醒的实际执行（由 main 注入到 on_wake 闭包）。
/// 解密 MAC → 发 magic packet → 上报 bemfa_wake。
#[allow(clippy::too_many_arguments)]
pub async fn trigger_wol(
    state: &SharedState,
    private_key: &rsa::RsaPrivateKey,
    wol_settings: &crate::config::WolSettings,
    did: &str,
    http_client: &reqwest::Client,
    server_url: &str,
    pairing_code: &str,
) {
    let device = {
        let s = state.read().expect("state lock");
        s.devices.get(did).cloned()
    };
    let device = if let Some(d) = device {
        d
    } else {
        tracing::warn!(%did, "bemfa wake: device not found in state");
        let _ = crate::reporter::wake(
            http_client,
            server_url,
            pairing_code,
            wakewake_protocol::WakeRequest {
                did: did.to_string(),
                success: false,
                message: "device not found".into(),
            },
        )
        .await;
        return;
    };
    let addr = wol_settings
        .broadcast_socket_addr()
        .unwrap_or_else(|_| "255.255.255.255:9".parse().expect("valid addr"));
    let result = wol::send_wol(
        &device.mac_encrypted,
        private_key,
        addr,
        wol_settings.packet_count,
        std::time::Duration::from_millis(wol_settings.packet_delay_ms),
    )
    .await;
    let (success, message) = match result {
        Ok(msg) => (true, msg),
        Err(msg) => (false, msg),
    };
    let _ = crate::reporter::wake(
        http_client,
        server_url,
        pairing_code,
        wakewake_protocol::WakeRequest {
            did: device.did.clone(),
            success,
            message,
        },
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6.1 golden test：固定 aid_simple 算 k4，锁定字面量值（agent 与 server 双方一致）。
    /// 预期值：sha256("550e8400e29b41d4a716446655440000")[..8] 大端 u64 = 1445437434999184868
    ///         → base36 小写 = "azcct2o5vx0k" → 取前 4 = "azcc"。
    /// （server 端的 k4 派生在本仓库不实现——server 通过 state 快照投递 aid，
    /// agent 独立计算 topic；此 golden 锁定 agent 侧字节级实现，防止端序/大小写偏差。）
    #[test]
    fn k4_is_deterministic_for_fixed_aid() {
        let aid_simple = "550e8400e29b41d4a716446655440000";
        let k4 = compute_k4(aid_simple);
        assert_eq!(k4, "azcc");
        // 确定性：同输入同输出（隐含 len/charset 一致）
        assert_eq!(k4, compute_k4(aid_simple));
    }

    #[test]
    fn topic_naming_is_41_chars_and_owned() {
        let aid_simple = "550e8400e29b41d4a716446655440000";
        let did = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"; // 32 hex
        let topic = device_topic(aid_simple, did);
        assert_eq!(topic.len(), 2 + K4_LEN + DID_LEN + 3); // ww(2)+k4(4)+did(32)+006(3)=41
        assert!(topic.starts_with(TOPIC_PREFIX));
        assert!(topic.ends_with(DEVICE_TYPE_CODE));
        assert!(is_owned_topic(aid_simple, &topic));
    }

    #[test]
    fn parse_did_round_trip() {
        let aid_simple = "550e8400e29b41d4a716446655440000";
        let did = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";
        let topic = device_topic(aid_simple, did);
        assert_eq!(
            parse_did_from_topic(aid_simple, &topic).as_deref(),
            Some(did)
        );
    }

    #[test]
    fn parse_did_rejects_wrong_k4_prefix() {
        // 不同 aid（不同部署）的 topic 不属于本部署
        let aid1 = "550e8400e29b41d4a716446655440000";
        let aid2 = "11111111111111111111111111111111";
        let did = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";
        let topic = device_topic(aid1, did);
        // 若 aid1 != aid2 的 k4 不同，则 aid2 解析 aid1 的 topic 应失败
        if compute_k4(aid1) != compute_k4(aid2) {
            assert!(parse_did_from_topic(aid2, &topic).is_none());
            assert!(!is_owned_topic(aid2, &topic));
        }
    }

    #[test]
    fn parse_did_rejects_wrong_type_code() {
        let aid_simple = "550e8400e29b41d4a716446655440000";
        let did = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";
        let k4 = compute_k4(aid_simple);
        let bad_topic = format!("{TOPIC_PREFIX}{k4}{did}002"); // 错误类型码
        assert!(parse_did_from_topic(aid_simple, &bad_topic).is_none());
    }

    #[test]
    fn to_base36_lower_known_values() {
        assert_eq!(to_base36_lower(0), "0");
        assert_eq!(to_base36_lower(35), "z");
        assert_eq!(to_base36_lower(36), "10");
        // 全小写
        assert!(
            to_base36_lower(123_456_789)
                .chars()
                .all(|c: char| c.is_ascii_digit() || c.is_ascii_lowercase())
        );
    }
}
