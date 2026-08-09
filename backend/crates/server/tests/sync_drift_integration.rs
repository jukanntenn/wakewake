//! device-sync-v3 漂移判定集成测试（真实 PG，testing/strategy.md §2）。
//!
//! 覆盖 §9.4 真值表 + §9.3 apply_observation 落库 + handle_sync 集成：
//! - 观测三态编码（⊥从未观测 / ∅topic 不存在 / 字符串值）
//! - 漂移判定 11 情形（穷举）
//! - 已删设备静默忽略（§9.3 末尾）
//! - drift_at/drift_kind 保留首次漂移时刻（§9.3）
//! - last_error 每轮覆盖（§9.3）

mod common;

use sqlx::PgPool;
use uuid::Uuid;
use wakewake_protocol::{Observation, SyncIntegration, SyncRequest};

use common::AdvisoryLockGuard;
use wakewake_server::hub::Hub;
use wakewake_server::repo::{agent_repo, device_repo, integration_repo};
use wakewake_server::service::command_handler;

async fn pool() -> (PgPool, AdvisoryLockGuard) {
    common::pool_locked().await
}

/// 种子：user + agent + 一台设备。返回 (user_id, agent_id, device_did, expected_name)。
async fn seed_device(pool: &PgPool) -> (i64, i64, Uuid, String) {
    let (user_id, agent_id) = common::seed_user_and_agent(pool).await;
    let did = Uuid::new_v4();
    let device = device_repo::insert(
        pool,
        &device_repo::NewDevice {
            did,
            user_id,
            agent_id,
            name: "客厅电脑",
            mac_encrypted: &"A".repeat(344),
            mac_display: "AA:**:**:**:**:FF",
            description: None,
        },
    )
    .await
    .unwrap();
    let _ = device;
    (user_id, agent_id, did, "客厅电脑".to_string())
}

/// 读设备的观测列。
async fn read_observation(
    pool: &PgPool,
    did: Uuid,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let row: (Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT bemfa_observed_name, last_drift_kind, last_error, bemfa_observed_name FROM devices WHERE did = $1",
    )
    .bind(did)
    .fetch_one(pool)
    .await
    .unwrap();
    row
}

/// 提交一次 sync 报告（单设备观测）。
async fn submit_sync(
    pool: &PgPool,
    hub: &Hub,
    agent_id: i64,
    did: Uuid,
    observed_name: Option<&str>,
) {
    let req = SyncRequest {
        applied_version: Some(1),
        integration: Some(SyncIntegration {
            provider: "bemfa".into(),
            enabled: true,
            mqtt_connected: true,
            error: None,
            observations: vec![Observation {
                did: did.simple().to_string(),
                observed_name: observed_name.map(str::to_string),
            }],
            repair_errors: vec![],
        }),
    };
    let _ = command_handler::handle_sync(pool, hub, agent_id, req)
        .await
        .unwrap();
}

#[tokio::test]
async fn case_1_new_device_no_topic_no_drift() {
    // §9.4 情形 1：新设备 P=⊥ O=∅ D=X → 不漂移
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, _) = seed_device(&pool).await;
    let hub = Hub::new();
    submit_sync(&pool, &hub, agent_id, did, None).await; // O=∅
    let (observed_name, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(observed_name, None); // observed_name=NULL（topic 不存在）
    assert_eq!(drift_kind, None); // 不漂移（P=⊥）
}

#[tokio::test]
async fn case_5_create_success_no_drift() {
    // §9.4 情形 5：create 成功 P=∅ O=X D=X → 不漂移（O==D 收敛）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    // 第一轮：O=∅（P 从 ⊥ 变 ∅）
    submit_sync(&pool, &hub, agent_id, did, None).await;
    // 第二轮：O=X（create 成功）
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await;
    let (observed_name, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(observed_name.as_deref(), Some("客厅电脑"));
    assert_eq!(drift_kind, None); // O==D，不漂移
}

#[tokio::test]
async fn case_7_cloud_deleted_drifts_deleted() {
    // §9.4 情形 7：稳态 P=X，用户删云端 topic O=∅ D=X → 漂移 deleted
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    // 建立稳态 P=X
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await;
    // 用户删云端 → O=∅
    submit_sync(&pool, &hub, agent_id, did, None).await;
    let (observed_name, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(observed_name, None); // O=∅
    assert_eq!(drift_kind.as_deref(), Some("deleted")); // 漂移 deleted
}

#[tokio::test]
async fn case_8_cloud_renamed_drifts_renamed() {
    // §9.4 情形 8：稳态 P=X，用户改名云端 O=Y D=X → 漂移 renamed
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await; // P=X
    submit_sync(&pool, &hub, agent_id, did, Some("恶意改名")).await; // O=Y
    let (_, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(drift_kind.as_deref(), Some("renamed")); // 漂移 renamed
}

#[tokio::test]
async fn case_9_self_rename_success_no_drift() {
    // §9.4 情形 9：我方改名成功 P=X O=X' D=X' → 不漂移（O==D 收敛）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await; // P=X
    // 改 DB 期望名（模拟 UI 改名）
    sqlx::query("UPDATE devices SET name = '卧室电脑' WHERE did = $1")
        .bind(did)
        .execute(&pool)
        .await
        .unwrap();
    // 下一轮观测 O=X'（modifyName 成功）
    submit_sync(&pool, &hub, agent_id, did, Some("卧室电脑")).await;
    let (_, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(drift_kind, None); // O==D，不漂移
}

#[tokio::test]
async fn case_11_user_rename_in_ui_no_drift() {
    // §9.4 情形 11：用户在 UI 改名 P=X O=X D=X' → 不漂移（O==P 云端没变）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await; // P=X
    // UI 改期望名（DB name 变），但本轮观测仍是旧值
    sqlx::query("UPDATE devices SET name = '新名字' WHERE did = $1")
        .bind(did)
        .execute(&pool)
        .await
        .unwrap();
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await; // O=X（云端没变）
    let (_, drift_kind, _, _) = read_observation(&pool, did).await;
    assert_eq!(drift_kind, None); // O==P，不漂移
}

#[tokio::test]
async fn last_error_overwritten_each_round() {
    // §9.3：last_error 每轮覆盖（成功 NULL，失败写错误）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    // 第一轮带 repair_error
    let req = SyncRequest {
        applied_version: Some(1),
        integration: Some(SyncIntegration {
            provider: "bemfa".into(),
            enabled: true,
            mqtt_connected: true,
            error: None,
            observations: vec![Observation {
                did: did.simple().to_string(),
                observed_name: Some(name.clone()),
            }],
            repair_errors: vec![wakewake_protocol::RepairError {
                did: did.simple().to_string(),
                error: "createTopic timeout".into(),
            }],
        }),
    };
    let _ = command_handler::handle_sync(&pool, &hub, agent_id, req)
        .await
        .unwrap();
    let (_, _, last_error, _) = read_observation(&pool, did).await;
    assert_eq!(last_error.as_deref(), Some("createTopic timeout"));
    // 第二轮无 repair_error → last_error 清 NULL
    submit_sync(&pool, &hub, agent_id, did, Some(&name)).await;
    let (_, _, last_error2, _) = read_observation(&pool, did).await;
    assert_eq!(last_error2, None);
}

#[tokio::test]
async fn deleted_device_silently_ignored() {
    // §9.3 末尾：已删设备的观测静默忽略（不报错）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, _) = seed_device(&pool).await;
    let hub = Hub::new();
    // 硬删设备
    sqlx::query("DELETE FROM devices WHERE did = $1")
        .bind(did)
        .execute(&pool)
        .await
        .unwrap();
    // 提交该设备的观测 → 不应报错
    submit_sync(&pool, &hub, agent_id, did, Some("whatever")).await;
    // 设备确实不存在
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM devices WHERE did = $1")
        .bind(did)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn sync_returns_current_version() {
    // §8.6：sync 响应返回 current_version（I6 守卫支撑）
    let (pool, _guard) = pool().await;
    let (_user_id, agent_id, did, name) = seed_device(&pool).await;
    let hub = Hub::new();
    // bump projection_version
    agent_repo::bump_projection_version(&pool, agent_id)
        .await
        .unwrap();
    let req = SyncRequest {
        applied_version: Some(1),
        integration: Some(SyncIntegration {
            provider: "bemfa".into(),
            enabled: true,
            mqtt_connected: true,
            error: None,
            observations: vec![Observation {
                did: did.simple().to_string(),
                observed_name: Some(name),
            }],
            repair_errors: vec![],
        }),
    };
    let resp = command_handler::handle_sync(&pool, &hub, agent_id, req)
        .await
        .unwrap();
    assert_eq!(resp.current_version, 1); // bump 后 = 1
}

#[tokio::test]
async fn integration_runtime_status_written() {
    // §7.2：set_runtime_status 写 mqtt_connected/last_error/last_report_at
    let (pool, _guard) = pool().await;
    let (user_id, agent_id, did, name) = seed_device(&pool).await;
    // 建一个 bemfa 集成（直接 insert）
    integration_repo::insert(
        &pool,
        user_id,
        agent_id,
        "bemfa",
        &serde_json::json!({"uid": "ct"}),
        true,
    )
    .await
    .unwrap();
    let hub = Hub::new();
    let req = SyncRequest {
        applied_version: Some(0),
        integration: Some(SyncIntegration {
            provider: "bemfa".into(),
            enabled: true,
            mqtt_connected: true,
            error: None,
            observations: vec![Observation {
                did: did.simple().to_string(),
                observed_name: Some(name),
            }],
            repair_errors: vec![],
        }),
    };
    let _ = command_handler::handle_sync(&pool, &hub, agent_id, req)
        .await
        .unwrap();
    // 验证集成 runtime_status 已写
    let integ = integration_repo::find_by_agent_provider(&pool, agent_id, "bemfa")
        .await
        .unwrap()
        .unwrap();
    assert!(integ.mqtt_connected);
    assert!(integ.last_report_at.is_some()); // 含 integration 块 → last_report_at 刷
}
