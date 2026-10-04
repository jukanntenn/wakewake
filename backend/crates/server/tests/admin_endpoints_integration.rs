//! Admin 新端点集成测试（ui-ux-risk-control §0.3/§9.8/§9.10）。
//! 验证 activity UNION / integrations 列表 / maintenance POST+GET 往返。
//! 直接测 repo 层（避开 HTTP router 装配复杂度），验证 SQL 正确性。

mod common;

use sqlx::PgPool;
use wakewake_server::repo::admin_repo;

async fn seed_admin_action(pool: &PgPool, actor_id: i64, action: &str, reason: Option<&str>) {
    let detail = serde_json::json!({ "reason": reason });
    sqlx::query("INSERT INTO admin_actions (actor_id, action, detail) VALUES ($1, $2, $3)")
        .bind(actor_id)
        .bind(action)
        .bind(sqlx::types::Json(&detail))
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn activity_union_returns_both_login_and_audit() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, _agent_id) = common::seed_user_and_agent(&pool).await;
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    // 插入一条 login_event + 一条 admin_action
    sqlx::query("INSERT INTO login_events (user_id, email, success, ip_address, user_agent, failure_code) VALUES ($1, $2, false, '203.0.113.8'::inet, 'curl/7.68.0', 'INVALID_CREDENTIALS')")
        .bind(user_id)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    seed_admin_action(&pool, user_id, "user.disable", Some("Abuse")).await;

    // 查询：不传 kind → 两者都返回
    let rows = admin_repo::list_activity(&pool, None, None, None, None, None, 1, 50)
        .await
        .unwrap();
    let kinds: Vec<_> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert!(kinds.contains(&"login"), "should contain login event");
    assert!(kinds.contains(&"audit"), "should contain audit event");

    // login 事件应有 IP + failure_code
    let login_row = rows.iter().find(|r| r.kind == "login").unwrap();
    assert_eq!(login_row.detail_ip.as_deref(), Some("203.0.113.8"));
    assert_eq!(
        login_row.detail_failure_code.as_deref(),
        Some("INVALID_CREDENTIALS")
    );
    assert_eq!(login_row.action, "login_failed");

    // audit 事件应有 reason
    let audit_row = rows.iter().find(|r| r.kind == "audit").unwrap();
    assert_eq!(audit_row.action, "user.disable");
    assert_eq!(audit_row.detail_reason.as_deref(), Some("Abuse"));
}

#[tokio::test]
async fn activity_filter_kind_login_only() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, _agent_id) = common::seed_user_and_agent(&pool).await;
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO login_events (user_id, email, success) VALUES ($1, $2, true)")
        .bind(user_id)
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    seed_admin_action(&pool, user_id, "user.enable", None).await;

    let rows = admin_repo::list_activity(&pool, Some("login"), None, None, None, None, 1, 50)
        .await
        .unwrap();
    assert!(rows.iter().all(|r| r.kind == "login"));
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn activity_filter_result_failed() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("actres_{}@example.com", uuid::Uuid::new_v4());
    sqlx::query("INSERT INTO login_events (email, success) VALUES ($1, true), ($1, false)")
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();

    let rows = admin_repo::list_activity(
        &pool,
        Some("login"),
        Some("failed"),
        None,
        None,
        None,
        1,
        50,
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].action, "login_failed");
}

#[tokio::test]
async fn activity_search_by_email_prefix() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("searchprefix_{}@example.com", uuid::Uuid::new_v4());
    sqlx::query("INSERT INTO login_events (email, success) VALUES ($1, true)")
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    // 搜索 "searchprefix" 应命中
    let rows = admin_repo::list_activity(
        &pool,
        Some("login"),
        None,
        Some("searchprefix"),
        None,
        None,
        1,
        50,
    )
    .await
    .unwrap();
    assert!(rows.iter().any(|r| r.actor_label == email));
}

#[tokio::test]
async fn integrations_list_returns_rows_with_derived_status() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, agent_id) = common::seed_user_and_agent(&pool).await;
    // 插入一个 connected 态集成（enabled, mqtt_connected, last_report_at 有值）
    sqlx::query("INSERT INTO integrations (user_id, agent_id, provider, config, enabled, mqtt_connected, last_report_at) VALUES ($1, $2, 'bemfa', '{}', true, true, now())")
        .bind(user_id)
        .bind(agent_id)
        .execute(&pool)
        .await
        .unwrap();

    let rows = admin_repo::list_all_integrations(&pool, None, None, 1, 50)
        .await
        .unwrap();
    assert!(
        rows.iter()
            .any(|r| r.provider == "bemfa" && r.enabled && r.mqtt_connected)
    );
    let total = admin_repo::count_all_integrations(&pool, None, None)
        .await
        .unwrap();
    assert!(total >= 1);
}

#[tokio::test]
async fn integrations_search_by_email() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, agent_id) = common::seed_user_and_agent(&pool).await;
    sqlx::query("INSERT INTO integrations (user_id, agent_id, provider, config, enabled) VALUES ($1, $2, 'bemfa', '{}', true)")
        .bind(user_id)
        .bind(agent_id)
        .execute(&pool)
        .await
        .unwrap();
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let prefix = &email[..3.min(email.len())];
    let rows = admin_repo::list_all_integrations(&pool, None, Some(prefix), 1, 50)
        .await
        .unwrap();
    assert!(rows.iter().any(|r| r.user_email == email));
}

#[tokio::test]
async fn wakes_offset_pagination_with_filters() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, _agent_id) = common::seed_user_and_agent(&pool).await;
    // 插入 3 条 wake：2 manual success, 1 bemfa_wake failed
    for _ in 0..2 {
        sqlx::query("INSERT INTO wakes (user_id, device_did, device_name, type, status) VALUES ($1, $2, 'PC', 'wol', 'success')")
            .bind(user_id)
            .bind(uuid::Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO wakes (user_id, device_did, device_name, type, status) VALUES ($1, $2, 'NAS', 'bemfa_wake', 'failed')")
        .bind(user_id)
        .bind(uuid::Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();

    // type 过滤 wol → 2 条
    let rows =
        admin_repo::list_all_wakes_offset(&pool, None, None, Some("wol"), None, None, None, 1, 50)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.r#type == "wol"));

    // result 过滤 failed → 1 条
    let rows = admin_repo::list_all_wakes_offset(
        &pool,
        None,
        None,
        None,
        Some("failed"),
        None,
        None,
        1,
        50,
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "failed");

    // total 计数
    let total = admin_repo::count_all_wakes_offset(&pool, None, None, None, None, None, None)
        .await
        .unwrap();
    assert!(total >= 3);
}

#[tokio::test]
async fn maintenance_post_get_round_trip() {
    // 此测试验证 repo 层 maintenance handle 往返（HTTP handler 测试在 e2e）。
    use wakewake_server::config::MaintenanceMode;
    use wakewake_server::service::maintenance::MaintenanceHandle;

    let path = std::env::temp_dir().join(format!(
        "wakewake_admin_maint_test_{}.json",
        uuid::Uuid::new_v4()
    ));
    let h = MaintenanceHandle::load_or_init(
        false,
        MaintenanceMode::RegistrationDisabled,
        "",
        path.clone(),
    );

    // set → readonly
    h.set(true, MaintenanceMode::Readonly, "ro".into(), Some(1))
        .unwrap();
    assert!(h.enabled());
    assert_eq!(h.mode(), MaintenanceMode::Readonly);

    // set → disabled
    h.set(false, MaintenanceMode::Full, "".into(), Some(1))
        .unwrap();
    assert!(!h.enabled());

    let _ = std::fs::remove_file(&path);
}
