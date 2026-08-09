//! device-sync-v3 reconcile 跨轮状态集成测试（wiremock 双 mock：wakewake server + 巴法云）。
//!
//! 覆盖 §10.7「本轮修复结果下轮 `repair_errors` 携带」+ §10.6 teardown 清空挂起错误。
//!
//! 这两个场景共用 `WAKEWAKE_BEMFA__API_BASE` env 缝隙（bemfa.rs 的 E2E mock 重定向），
//! env 是进程全局量——为避免并行测试互相覆盖 env，合并进**单个**串行测试函数。

use std::sync::Arc;

use base64::Engine;
use rsa::sha2::Sha256;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use tokio::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// env 是进程全局量，用此锁强制所有设置 `WAKEWAKE_BEMFA__API_BASE` 的测试串行
/// （避免并行测试互相覆盖 env）。用 tokio::sync::Mutex 以支持跨 await 持有。
static ENV_LOCK: Mutex<()> = Mutex::const_new(());

use wakewake_agent::bemfa_state::{BemfaCoordinator, ReconcilerDeps};
use wakewake_agent::config::WolSettings;
use wakewake_agent::state::AgentState;
use wakewake_protocol::{DeviceData, IntegrationData};

const AID_SIMPLE: &str = "550e8400e29b41d4a716446655440000";
const DID: &str = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";
const DEVICE_NAME: &str = "客厅电脑";
/// 32 位 hex 合法 uid（§13.5.1 新版格式）。
const UID_PLAINTEXT: &str = "00ad90fe27444dff9d5ee32b94c5ae08";

/// 生成 RSA-2048 密钥对。
fn gen_keypair() -> (RsaPrivateKey, RsaPublicKey) {
    let mut rng = rand::rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA keygen");
    let pub_key = RsaPublicKey::from(&priv_key);
    (priv_key, pub_key)
}

/// 用公钥加密 uid 明文 → base64（对齐前端 Web Crypto RSA-OAEP/SHA-256）。
fn encrypt_uid(pub_key: &RsaPublicKey, plaintext: &str) -> String {
    let mut rng = rand::rng();
    let ct = pub_key
        .encrypt(&mut rng, Oaep::<Sha256>::new(), plaintext.as_bytes())
        .expect("RSA encrypt");
    base64::engine::general_purpose::STANDARD.encode(ct)
}

/// 构造测试用 SharedState：bemfa 集成（uid 已加密）+ 1 设备 + aid/applied_version。
fn seed_state(pub_key: &RsaPublicKey, enabled: bool) -> Arc<std::sync::RwLock<AgentState>> {
    let state = AgentState::new_shared();
    {
        let mut s = state.write().expect("state lock");
        s.aid = Some(AID_SIMPLE.to_string());
        s.applied_version = 1;
        s.devices.insert(
            DID.to_string(),
            DeviceData {
                did: DID.to_string(),
                name: DEVICE_NAME.to_string(),
                mac_encrypted: "ignored".to_string(),
                description: None,
            },
        );
        s.integrations = vec![IntegrationData {
            provider: "bemfa".to_string(),
            config: serde_json::json!({ "uid": encrypt_uid(pub_key, UID_PLAINTEXT) }),
            enabled,
        }];
    }
    state
}

/// 构造 ReconcilerDeps（state/server_url 注入；其余用测试桩）。
fn make_deps(
    state: Arc<std::sync::RwLock<AgentState>>,
    priv_key: RsaPrivateKey,
    server_url: String,
) -> ReconcilerDeps {
    ReconcilerDeps {
        state,
        private_key: priv_key,
        wol_settings: WolSettings {
            broadcast_addr: "255.255.255.255:9".to_string(),
            packet_count: 3,
            packet_delay_ms: 100,
        },
        http_client: reqwest::Client::new(),
        server_url,
        pairing_code: "testpairingcode01".to_string(),
    }
}

/// 提取所有 `/api/v1/agents/self/sync` 请求体中的 `integration.repair_errors`。
fn extract_repair_errors(received: &[wiremock::Request]) -> Vec<serde_json::Value> {
    received
        .iter()
        .filter(|r| r.url.path() == "/api/v1/agents/self/sync")
        .filter_map(|r| {
            let v: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
            Some(
                v.get("integration")?
                    .get("repair_errors")?
                    .as_array()?
                    .clone(),
            )
        })
        .flatten()
        .collect()
}

/// §10.7 + §10.6 串行场景（合并避免 env 并发竞争）。
///
/// 1. 轮 1：createTopic 返回 40009（失败）→ repair_errors 挂起，断言 did + 错误码。
/// 2. 轮 2：挂起错误出现在本轮 `/sync` 请求体 `repair_errors` 中（跨轮携带）。
/// 3. 禁用集成 → 轮 3 走 teardown → 挂起的 repair_errors 被清空。
#[tokio::test]
async fn repair_errors_carry_over_then_teardown_clears() {
    let _env_guard = ENV_LOCK.lock().await;
    // ---- mock 巴法云：allTopic 恒空 + createTopic 恒 40009 ----
    let bemfa_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            // 真实结构：topic 数组嵌套在 data.data（对齐巴法生产 API）
            "code": 0,
            "data": {"data": []}
        })))
        .mount(&bemfa_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/createTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 40009})))
        .mount(&bemfa_server)
        .await;

    // ---- mock wakewake server /sync：恒 200 + current_version=1（I6 守卫通过）----
    let wakewake_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/agents/self/sync"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"current_version": 1})),
        )
        .mount(&wakewake_server)
        .await;

    // env 缝隙重定向 bemfa API → mock。安全：仅本测试二进制进程内，串行执行无竞争。
    std::env::set_var("WAKEWAKE_BEMFA__API_BASE", bemfa_server.uri());

    let (priv_key, pub_key) = gen_keypair();
    let state = seed_state(&pub_key, true);
    let coord = BemfaCoordinator::new();
    let deps = make_deps(state.clone(), priv_key, wakewake_server.uri());

    // ---- 阶段 1：轮 1 createTopic 失败 → 挂起 repair_error ----
    let settled1 = coord.reconcile_once(&deps).await;
    assert!(!settled1, "轮 1 有 create 失败 → all_settled 应为 false");
    let pending = coord.pending_repair_errors().await;
    assert_eq!(pending.len(), 1, "轮 1 应挂起 1 个 repair error");
    assert_eq!(pending[0].did, DID);
    assert!(
        pending[0].error.contains("40009"),
        "错误文案应含 code 40009，got: {}",
        pending[0].error
    );

    // ---- 阶段 2：轮 2 /sync 请求体携带轮 1 的 repair_error ----
    let _settled2 = coord.reconcile_once(&deps).await;
    let received = wakewake_server.received_requests().await.expect("recorded");
    let repair_errors = extract_repair_errors(&received);
    assert!(
        repair_errors
            .iter()
            .any(|r| r.get("did").and_then(|d| d.as_str()) == Some(DID)),
        "轮 2 的 /sync 应携带轮 1 的 repair_error（did={DID}），实际: {repair_errors:?}"
    );

    // ---- 阶段 3：禁用集成 → 轮 3 teardown 清空挂起 ----
    {
        let mut s = state.write().expect("state lock");
        for integ in &mut s.integrations {
            integ.enabled = false;
        }
    }
    let settled3 = coord.reconcile_once(&deps).await;
    assert!(settled3, "禁用集成 → INACTIVE → all_settled");
    assert!(
        coord.pending_repair_errors().await.is_empty(),
        "teardown 应清空挂起 repair_errors"
    );

    std::env::remove_var("WAKEWAKE_BEMFA__API_BASE");
}

/// Notify 句柄：notify_one 不 panic 即视为接线正确（不触达 env，可独立并行）。
#[tokio::test]
async fn coordinator_notify_is_wired() {
    let coord = BemfaCoordinator::new();
    coord.notify_one();
    tokio::task::yield_now().await;
    assert!(coord.pending_repair_errors().await.is_empty());
}

/// §10.9 / §12.4：孤儿删除开启后，云端残留的本部署 topic（ww‖k4 前缀但 did 不在投影）
/// 在 I6 守卫通过时被 deleteTopic 清理。
#[tokio::test]
async fn orphan_topic_deleted_when_enabled_and_guard_passes() {
    use wakewake_agent::bemfa::{self, device_topic};
    let _env_guard = ENV_LOCK.lock().await;

    // 孤儿 topic：同 aid（同 k4 前缀）但 did 不在 seed_state 设备集
    let orphan_did = "deadbeefdeadbeefdeadbeefdeadbeef";
    let orphan_topic = device_topic(AID_SIMPLE, orphan_did);
    // 当前设备的 topic（不应被删）
    let owned_topic = device_topic(AID_SIMPLE, DID);

    // allTopic mock：返回孤儿 + 当前设备 topic（真实结构 data.data 嵌套）
    let bemfa_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": 0,
            "data": {
                "data": [
                    {"topic": orphan_topic.clone(), "name": ""},
                    {"topic": owned_topic.clone(), "name": DEVICE_NAME},
                ]
            }
        })))
        .mount(&bemfa_server)
        .await;
    // deleteTopic mock：期望被调用（孤儿）。任何 POST /v1/deleteTopic 返回成功。
    let delete_mock = Mock::given(method("POST"))
        .and(path("/v1/deleteTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 0})))
        .expect(1)
        .mount_as_scoped(&bemfa_server)
        .await;

    // /sync mock：current_version=1 == applied_version=1（I6 守卫通过）
    let wakewake_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/agents/self/sync"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"current_version": 1})),
        )
        .mount(&wakewake_server)
        .await;

    std::env::set_var("WAKEWAKE_BEMFA__API_BASE", bemfa_server.uri());

    let (priv_key, pub_key) = gen_keypair();
    let state = seed_state(&pub_key, true);
    let coord = BemfaCoordinator::new();
    let deps = make_deps(state, priv_key, wakewake_server.uri());

    let settled = coord.reconcile_once(&deps).await;
    // all_settled：无 create（当前 topic 已存在）/ modify（name 匹配）/ repair_error
    assert!(settled, "稳态应 all_settled");

    // delete_mock 的 expect(1) 在 drop 时断言调用次数
    drop(delete_mock);
    std::env::remove_var("WAKEWAKE_BEMFA__API_BASE");

    // 静态断言 orphan_topic 确实是本部署前缀（is_owned_topic 为 true），否则测试无意义
    assert!(
        bemfa::is_owned_topic(AID_SIMPLE, &orphan_topic),
        "测试前提：orphan_topic 须是本部署前缀"
    );
}
