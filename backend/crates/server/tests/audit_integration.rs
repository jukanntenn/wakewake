//! 审计扩展集成测试（ui-ux-risk-control §8.5）。
//! 验证 change_password / rotate_pairing_code / disable_user 写 admin_actions 审计。

mod common;

use sqlx::PgPool;
use wakewake_server::config::{MaintenanceMode, Settings};
use wakewake_server::service::admin_service;
use wakewake_server::service::maintenance::MaintenanceHandle;
use wakewake_server::state::AppState;
use wakewake_server::{
    config::RateLimitSettings,
    hub::Hub,
    integrations::ProviderRegistry,
    service::{
        command_handler::WakeWriter, login_lockout::LoginLockout, mailer_service::MailerService,
        pow::PowService,
    },
};

fn test_settings() -> Settings {
    serde_json::from_str(
        r#"{
            "app": {"public_url": "https://example.com"},
            "server": {"host": "0.0.0.0", "port": 8080, "trust_proxy": true},
            "database": {"dsn": "unused"},
            "jwt": {"signing_key": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "refresh_signing_key": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "access_expire": "15m", "refresh_expire": "720h"},
            "password_reset": {"secret": "cccccccccccccccccccccccccccccccc", "expire": "1h"},
            "pow": {"difficulty": 4, "challenge_ttl": "10m"},
            "mailer": {"enabled": false, "smtp_port": 587, "from_name": "test"},
            "rate_limit": {"disabled": true},
            "security": {"bootstrap_admin_email": "a@b.c", "bootstrap_admin_password": "pass1234"},
            "log": {"level": "info", "dir": "data/logs"},
            "maintenance": {"enabled": false, "mode": "registration_disabled", "message": ""}
        }"#,
    )
    .unwrap()
}

async fn build_state(pool: PgPool) -> AppState {
    let settings = test_settings();
    let (wake_tx, _wake_rx) = tokio::sync::mpsc::channel(256);
    let tmp =
        || std::env::temp_dir().join(format!("wakewake_audit_test_{}.json", uuid::Uuid::new_v4()));
    let maintenance =
        MaintenanceHandle::load_or_init(false, MaintenanceMode::RegistrationDisabled, "", tmp());
    let mailer_control = wakewake_server::service::mailer_control::MailerControl::load_or_init(
        wakewake_server::service::mailer_control::MailLimits::default(),
        tmp(),
    );
    let ip_bans = wakewake_server::service::ip_ban::IpBanStore::load_or_init(tmp());
    AppState::new(
        pool,
        settings,
        Hub::new(),
        ProviderRegistry::build().unwrap(),
        MailerService::new(&test_settings().mailer, mailer_control.clone()),
        PowService::new(&test_settings().pow, None),
        LoginLockout::new(),
        wake_tx,
        maintenance,
        mailer_control,
        ip_bans,
    )
}

async fn count_actions(pool: &PgPool, actor_id: i64, action: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM admin_actions WHERE actor_id = $1 AND action = $2")
            .bind(actor_id)
            .bind(action)
            .fetch_one(pool)
            .await
            .unwrap();
    n
}

#[tokio::test]
async fn change_password_writes_audit() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("chpw_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("oldpass12", 4).unwrap();
    let (user_id,): (i64,) = sqlx::query_as(
        "INSERT INTO users (email, password, email_verified) VALUES ($1, $2, true) RETURNING id",
    )
    .bind(&email)
    .bind(&hash)
    .fetch_one(&pool)
    .await
    .unwrap();

    wakewake_server::service::auth_service::change_password(
        &pool,
        user_id,
        "oldpass12",
        "newpass123",
    )
    .await
    .unwrap();

    let n = count_actions(&pool, user_id, "user.change_password").await;
    assert_eq!(n, 1, "user.change_password audit should be written");
}

#[tokio::test]
async fn disable_user_writes_reason_and_audit() {
    let (pool, _g) = common::pool_locked().await;
    let state = build_state(pool.clone()).await;
    let email = format!("disablereason_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("pass1234", 4).unwrap();
    let (user_id,): (i64,) = sqlx::query_as(
        "INSERT INTO users (email, password, email_verified) VALUES ($1, $2, true) RETURNING id",
    )
    .bind(&email)
    .bind(&hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (admin_id,): (i64,) =
        sqlx::query_as("INSERT INTO users (email, password, email_verified, is_superuser) VALUES ($1, $2, true, true) RETURNING id")
            .bind(format!("admin_{}@example.com", uuid::Uuid::new_v4()))
            .bind(hash)
            .fetch_one(&pool)
            .await
            .unwrap();

    let user = admin_service::disable_user(&state, admin_id, user_id, Some("Abuse"))
        .await
        .unwrap();

    // disabled_reason + disabled_by 写入 users 表
    assert_eq!(user.disabled_reason.as_deref(), Some("Abuse"));
    assert_eq!(user.disabled_by, Some(admin_id));

    // admin_actions 记 user.disable，detail 含 reason
    let (action,): (String,) = sqlx::query_as(
        "SELECT action FROM admin_actions WHERE actor_id = $1 AND target_user_id = $2",
    )
    .bind(admin_id)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "user.disable");

    // enable 后清空 disabled_reason/by
    let _ = admin_service::enable_user(&state, admin_id, user_id)
        .await
        .unwrap();
    let (reason,): (Option<String>,) =
        sqlx::query_as("SELECT disabled_reason FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        reason.is_none(),
        "disabled_reason should be cleared on enable"
    );
}

#[tokio::test]
async fn rotate_pairing_code_writes_audit() {
    let (pool, _g) = common::pool_locked().await;
    let (user_id, agent_id) = common::seed_user_and_agent(&pool).await;

    wakewake_server::service::agent_service::rotate_pairing_code(&pool, user_id)
        .await
        .unwrap();

    let n = count_actions(&pool, user_id, "agent.rotate_code").await;
    assert_eq!(n, 1, "agent.rotate_code audit should be written");
    // target_agent_id 应指向该 agent
    let (target_agent,): (Option<i64>,) =
        sqlx::query_as("SELECT target_agent_id FROM admin_actions WHERE actor_id = $1 AND action = 'agent.rotate_code'")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_agent, Some(agent_id));
}
