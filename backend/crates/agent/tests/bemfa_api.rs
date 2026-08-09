//! device-sync-v3 bemfa HTTP API 集成测试（wiremock mock 巴法云 HTTP）。
//!
//! 覆盖 §6.1 topic 命名 + §13.5 allTopic 原始 uid（对齐 ha-xiaodu 生产）+ §13.2 createTopic 幂等。

use base64::Engine;
use wakewake_agent::bemfa::{
    CreateTopicParams, V2Credentials, compute_k4, device_topic, is_owned_topic,
    parse_did_from_topic,
};
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const AID_SIMPLE: &str = "550e8400e29b41d4a716446655440000";
const DID: &str = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";
const UID: &str = "00ad90fe27444dff9d5ee32b94c5ae08";

fn client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}

// ---- §6.1 topic 命名（纯函数，golden）----

#[test]
fn k4_is_4_chars_lowercase_alphanumeric() {
    let k4 = compute_k4(AID_SIMPLE);
    assert_eq!(k4.len(), 4);
    assert!(
        k4.chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    );
}

#[test]
fn device_topic_structure_ww_k4_did_006() {
    let topic = device_topic(AID_SIMPLE, DID);
    assert!(topic.starts_with("ww"));
    assert!(topic.ends_with("006"));
    assert!(topic.contains(DID));
    // 总长度 = ww(2) + k4(4) + did(32) + 006(3) = 41
    assert_eq!(topic.len(), 41);
}

#[test]
fn parse_did_round_trip() {
    let topic = device_topic(AID_SIMPLE, DID);
    assert_eq!(
        parse_did_from_topic(AID_SIMPLE, &topic).as_deref(),
        Some(DID)
    );
}

#[test]
fn is_owned_topic_matches_own_k4_prefix() {
    let topic = device_topic(AID_SIMPLE, DID);
    assert!(is_owned_topic(AID_SIMPLE, &topic));
    // 不同 aid（不同 k4）的 topic 不属于本部署
    let other_aid = "11111111111111111111111111111111";
    if compute_k4(AID_SIMPLE) != compute_k4(other_aid) {
        assert!(!is_owned_topic(other_aid, &topic));
    }
}

// ---- §13.2 createTopic（v1/v2 + 40006 幂等）----

#[tokio::test]
async fn create_topic_v1_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/createTopic"))
        .and(body_partial_json(
            serde_json::json!({"uid": UID, "topic": device_topic(AID_SIMPLE, DID), "type": 1}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 0})))
        .expect(1)
        .mount(&server)
        .await;

    let params = CreateTopicParams {
        uid: UID,
        topic: &device_topic(AID_SIMPLE, DID),
        name: Some("客厅电脑"),
        v2: None,
    };
    let url = format!("{}/v1/createTopic", server.uri());
    let result = wakewake_agent::bemfa::create_topic_at(&client(), &url, &params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn create_topic_40006_idempotent_success() {
    // §13.2：40006（设备已存在）视为幂等成功
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/createTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 40006})))
        .mount(&server)
        .await;

    let params = CreateTopicParams {
        uid: UID,
        topic: &device_topic(AID_SIMPLE, DID),
        name: None,
        v2: None,
    };
    let url = format!("{}/v1/createTopic", server.uri());
    let result = wakewake_agent::bemfa::create_topic_at(&client(), &url, &params).await;
    assert!(result.is_ok(), "40006 should be idempotent success");
}

#[tokio::test]
async fn create_topic_40009_permanent_error() {
    // §13.2：40009（主题错误）= 永久失败
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/createTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 40009})))
        .mount(&server)
        .await;

    let params = CreateTopicParams {
        uid: UID,
        topic: &device_topic(AID_SIMPLE, DID),
        name: None,
        v2: None,
    };
    let url = format!("{}/v1/createTopic", server.uri());
    let result = wakewake_agent::bemfa::create_topic_at(&client(), &url, &params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn create_topic_v2_includes_secret_credentials() {
    // v2 凭证成对时走 v2 URL + body 含 secretID/secretKey
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vs/web/v2/createTopic"))
        .and(body_partial_json(
            serde_json::json!({"secretID": "sid", "secretKey": "skey"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 0})))
        .expect(1)
        .mount(&server)
        .await;

    let v2 = V2Credentials {
        secret_id: "sid".into(),
        secret_key: "skey".into(),
    };
    let params = CreateTopicParams {
        uid: UID,
        topic: &device_topic(AID_SIMPLE, DID),
        name: None,
        v2: Some(&v2),
    };
    let url = format!("{}/vs/web/v2/createTopic", server.uri());
    let result = wakewake_agent::bemfa::create_topic_at(&client(), &url, &params).await;
    assert!(result.is_ok());
}

// ---- §13.5 allTopic（原始 uid，对齐 ha-xiaodu 生产，偏离 §13.5 base64 断言）----

#[tokio::test]
async fn all_topic_uses_raw_uid_as_openid() {
    // 关键断言：agent 发的 allTopic 请求 openID = 原始 uid（明文，非 base64）。
    // 对齐 ha-xiaodu 生产 api_client.py:241 + 巴法文档示例。
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .and(query_param("openID", UID)) // 原始 uid，非 base64
        .and(query_param("type", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            // 真实结构：topic 数组嵌套在 data.data（对齐巴法生产 API，非 .local 旧文档）
            "code": 0,
            "msg": "success",
            "data": {"data": [{"topic": device_topic(AID_SIMPLE, DID), "name": "客厅电脑"}]}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let url = format!("{}/vb/api/v2/allTopic", server.uri());
    let result = wakewake_agent::bemfa::list_all_topics_detail_at(&client(), &url, UID).await;
    assert!(result.is_ok());
    let topics = result.unwrap();
    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].name.as_deref(), Some("客厅电脑"));
}

#[tokio::test]
async fn all_topic_does_not_send_base64_uid() {
    // 反向断言：确保 agent 没有把 uid base64 编码后作为 openID
    let server = MockServer::start().await;
    let encoded = base64::engine::general_purpose::STANDARD.encode(UID.as_bytes());
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            // 真实结构：data 是对象（即便为空也是 data.data:[]）
            "code": 0,
            "data": {"data": []}
        })))
        .mount(&server)
        .await;

    let url = format!("{}/vb/api/v2/allTopic", server.uri());
    let _ = wakewake_agent::bemfa::list_all_topics_detail_at(&client(), &url, UID).await;

    // 取实际请求，断言 openID 是原始 uid 而非 base64
    let received = &server.received_requests().await.unwrap()[0];
    let req_url = format!("{}", received.url);
    assert!(
        req_url.contains(&format!("openID={UID}")),
        "openID should be raw uid: {req_url}"
    );
    assert!(
        !req_url.contains(&format!("openID={encoded}")),
        "openID must not be base64: {req_url}"
    );
}

// ---- allTopic 多态 data 形态（真实巴法 API 行为）----

#[tokio::test]
async fn all_topic_handles_empty_string_data_on_failure() {
    // 巴法失败响应：data 是空字符串 ""，不应导致反序列化 panic/Err(parse)，
    // 应走 code != SUCCESS 分支返回业务错误。
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": -1,
            "msg": "获取失败",
            "data": ""
        })))
        .mount(&server)
        .await;

    let url = format!("{}/vb/api/v2/allTopic", server.uri());
    let result = wakewake_agent::bemfa::list_all_topics_detail_at(&client(), &url, UID).await;
    assert!(result.is_err());
    assert!(
        result.unwrap_err().contains("code -1"),
        "应报业务码而非 parse 错"
    );
}

#[tokio::test]
async fn all_topic_error_message_includes_body_on_parse_failure() {
    // 反序列化失败时，错误信息应含原始 body 片段（层1 可观测性回归门）。
    // 下次巴法改 API，日志直接给出真实响应 + HTTP 状态，无需手动 curl。
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/vb/api/v2/allTopic"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("not json at all <html>500</html>"),
        )
        .mount(&server)
        .await;

    let url = format!("{}/vb/api/v2/allTopic", server.uri());
    let result = wakewake_agent::bemfa::list_all_topics_detail_at(&client(), &url, UID).await;
    let err = result.unwrap_err();
    assert!(err.contains("body_head"), "错误应带 body 诊断片段: {err}");
    assert!(
        err.contains("not json at all"),
        "错误应含原始响应文本: {err}"
    );
}

/// 真实巴法云契约探针（默认不跑）。用文档公开示例 uid 验证生产 API 结构
/// 与代码 `AllTopicResponse` 一致。一旦巴法改结构会红，提醒更新代码 + `.local/bemfa` 文档。
/// 手动运行：`cargo test -- --ignored bemfa_real_contract_probe`
/// 网络依赖 + 非确定性，故 ignored。
#[tokio::test]
#[ignore = "network-dependent + non-deterministic; run manually: cargo test -- --ignored bemfa_real_contract_probe"]
async fn bemfa_real_contract_probe() {
    let client = reqwest::Client::new();
    let uid = "4d9ec352e0376f2110a0c601a2857225"; // .local/bemfa/api_device.md 公开示例 uid
    let result = wakewake_agent::bemfa::list_all_topics_detail(&client, uid).await;
    assert!(result.is_ok(), "巴法 API 结构漂移！err: {result:?}");
}

// ---- §13.3 deleteTopic / §13.4 modifyName ----

#[tokio::test]
async fn delete_topic_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/deleteTopic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 0})))
        .expect(1)
        .mount(&server)
        .await;

    let url = format!("{}/v1/deleteTopic", server.uri());
    let result = wakewake_agent::bemfa::delete_topic_at(&client(), &url, UID, "sometopic006").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn modify_name_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/va/modifyName"))
        .and(body_partial_json(serde_json::json!({"name": "新名字"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"code": 0})))
        .expect(1)
        .mount(&server)
        .await;

    let url = format!("{}/va/modifyName", server.uri());
    let result =
        wakewake_agent::bemfa::modify_name_at(&client(), &url, UID, "topic006", "新名字").await;
    assert!(result.is_ok());
}
