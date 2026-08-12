//! login_events 集成测试（ui-ux-risk-control §8.3/§11.H.2）。
//!
//! 验证 auth_service::login() 在各路径正确写 login_events。
//! 需真实 PG（testing/strategy.md §2，不 mock DB）。

mod common;

use sqlx::PgPool;
use wakewake_server::config::{MaintenanceMode, Settings};
use wakewake_server::service::auth_service::{self, LoginAudit};
use wakewake_server::service::login_lockout::LoginLockout;
use wakewake_server::service::maintenance::MaintenanceHandle;

/// 查询某 email 的 login_events（按时间倒序）。
async fn events_for_email(pool: &PgPool, email: &str) -> Vec<(bool, Option<String>)> {
    let rows: Vec<(bool, Option<String>)> = sqlx::query_as(
        "SELECT success, failure_code FROM login_events WHERE email = $1 ORDER BY created_at DESC",
    )
    .bind(email)
    .fetch_all(pool)
    .await
    .expect("query login_events");
    rows
}

/// 构造测试用 Settings（jwt 密钥合法长度）。
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
    .expect("parse test settings")
}

fn maintenance_disabled() -> MaintenanceHandle {
    MaintenanceHandle::load_or_init(
        false,
        MaintenanceMode::RegistrationDisabled,
        "",
        std::env::temp_dir().join(format!("wakewake_test_maint_{}.json", uuid::Uuid::new_v4())),
    )
}

#[tokio::test]
async fn login_success_writes_success_event() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("success_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("password123", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified) VALUES ($1, $2, true)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = maintenance_disabled();
    let audit = LoginAudit::default();

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "password123",
        &audit,
    )
    .await;
    assert!(res.is_ok(), "login should succeed");

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, true); // success
    assert_eq!(events[0].1, None); // no failure_code
}

#[tokio::test]
async fn login_wrong_password_writes_invalid_credentials() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("wrongpw_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("correctpass", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified) VALUES ($1, $2, true)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = maintenance_disabled();

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "wrongpass",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_err());

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, false);
    assert_eq!(events[0].1.as_deref(), Some("INVALID_CREDENTIALS"));
}

#[tokio::test]
async fn login_nonexistent_user_writes_event_with_null_user_id() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("nonexistent_{}@example.com", uuid::Uuid::new_v4());

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = maintenance_disabled();

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "anypass",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_err());

    // user_id 应为 NULL（撞库）
    let (user_id,): (Option<i64>,) =
        sqlx::query_as("SELECT user_id FROM login_events WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        user_id.is_none(),
        "nonexistent user should have null user_id"
    );

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events[0].1.as_deref(), Some("INVALID_CREDENTIALS"));
}

#[tokio::test]
async fn login_disabled_user_writes_user_disabled() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("disabled_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("password123", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified, is_active) VALUES ($1, $2, true, false)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = maintenance_disabled();

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "password123",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_err());

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events[0].0, false);
    assert_eq!(events[0].1.as_deref(), Some("USER_DISABLED"));
}

#[tokio::test]
async fn login_unverified_email_writes_email_not_verified() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("unverified_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("password123", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified) VALUES ($1, $2, false)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = maintenance_disabled();

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "password123",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_err());

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events[0].0, false);
    assert_eq!(events[0].1.as_deref(), Some("EMAIL_NOT_VERIFIED"));
}

#[tokio::test]
async fn full_maintenance_blocks_non_admin_login() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("fullmaint_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("password123", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified) VALUES ($1, $2, true)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = MaintenanceHandle::load_or_init(
        true,
        MaintenanceMode::Full,
        "test",
        std::env::temp_dir().join(format!(
            "wakewake_test_maint_full_{}.json",
            uuid::Uuid::new_v4()
        )),
    );

    // 非 admin 用户：密码正确但因 full 模式被拒（共识 2）
    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "password123",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_err());

    let events = events_for_email(&pool, &email).await;
    assert_eq!(events[0].0, false);
    assert_eq!(events[0].1.as_deref(), Some("MAINTENANCE_FULL"));
}

#[tokio::test]
async fn full_maintenance_allows_admin_login() {
    let (pool, _g) = common::pool_locked().await;
    let email = format!("admin_{}@example.com", uuid::Uuid::new_v4());
    let hash = bcrypt::hash("password123", 4).unwrap();
    sqlx::query("INSERT INTO users (email, password, email_verified, is_superuser) VALUES ($1, $2, true, true)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let settings = test_settings();
    let lockout = LoginLockout::new();
    let maint = MaintenanceHandle::load_or_init(
        true,
        MaintenanceMode::Full,
        "test",
        std::env::temp_dir().join(format!(
            "wakewake_test_maint_admin_{}.json",
            uuid::Uuid::new_v4()
        )),
    );

    let res = auth_service::login(
        &pool,
        &settings,
        &lockout,
        &maint,
        &email,
        "password123",
        &LoginAudit::default(),
    )
    .await;
    assert!(res.is_ok(), "admin should login in full maintenance mode");
}

#[tokio::test]
async fn login_events_cleanup_deletes_old_records() {
    let (pool, _g) = common::pool_locked().await;
    // 插入一条 31 天前的记录 + 一条最近记录
    sqlx::query("INSERT INTO login_events (email, success, created_at) VALUES ('old@test.com', false, now() - interval '31 days')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO login_events (email, success) VALUES ('recent@test.com', true)")
        .execute(&pool)
        .await
        .unwrap();

    // 模拟清理任务 SQL
    sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '30 days'")
        .execute(&pool)
        .await
        .unwrap();

    let (old_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM login_events WHERE email = 'old@test.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (new_count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM login_events WHERE email = 'recent@test.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_count, 0, "31-day-old record should be deleted");
    assert_eq!(new_count, 1, "recent record should remain");
}
