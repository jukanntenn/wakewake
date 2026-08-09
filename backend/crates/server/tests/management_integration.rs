//! management 模块集成测试（真实 PG，testing/strategy.md §2）。
//!
//! 覆盖 admin 补 agent 的三条路径（功能点 2）：
//! - `ensure_bootstrap_admin`：新建 admin → user + agent；老 admin（无 agent）→ 补偿；已有 agent → 幂等
//! - CLI `admin create`：建 user + agent
//! - CLI `admin promote`：已有 agent 保持；无 agent 补建（防御）
//!
//! 核心：admin 创建路径必须与 `auth_service::register` 对称——register 建 user + agent，
//! admin 路径之前只建 user 不建 agent，导致 admin 无法使用设备/唤醒/集成等需 agent 的功能。

mod common;

use sqlx::PgPool;
use wakewake_server::config::SecuritySettings;
use wakewake_server::management;
use wakewake_server::repo::{agent_repo, user_repo};
use wakewake_server::service::agent_service;

use common::AdvisoryLockGuard;

async fn pool() -> (PgPool, AdvisoryLockGuard) {
    common::pool_locked().await
}

fn security(email: &str) -> SecuritySettings {
    SecuritySettings {
        bootstrap_admin_email: email.into(),
        bootstrap_admin_password: "TestPass123!".into(),
    }
}

/// 断言 user 拥有恰好一个 agent。
async fn assert_has_one_agent(pool: &PgPool, user_id: i64) {
    let count = agent_repo::count_for_update(pool, user_id).await.unwrap();
    assert_eq!(count, 1, "user {user_id} should have exactly one agent");
}

// ── ensure_bootstrap_admin ──────────────────────────────────────────────

#[tokio::test]
async fn bootstrap_creates_new_admin_with_agent() {
    let (pool, _g) = pool().await;
    let email = "bootstrap_new@example.com";
    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();

    let user = user_repo::find_by_email(&pool, email).await.unwrap();
    let user = user.expect("admin user created");
    assert!(user.is_superuser, "bootstrap admin should be superuser");
    assert!(
        user.email_verified,
        "bootstrap admin email should be verified"
    );
    assert_has_one_agent(&pool, user.id).await;
}

#[tokio::test]
async fn bootstrap_compensates_missing_agent() {
    let (pool, _g) = pool().await;
    let email = "bootstrap_old@example.com";
    // 模拟老版本 admin：手动插 user（superuser）但不建 agent
    let dummy_hash = "$2b$10$abcdefghijklmnopqrstuvABCDEFGHIJKLMNOPQRSTUVWXY";
    let user = user_repo::insert_verified(&pool, email, dummy_hash, true)
        .await
        .unwrap();
    // 确认此时无 agent
    assert!(
        agent_repo::find_by_user(&pool, user.id)
            .await
            .unwrap()
            .is_none(),
        "precondition: old admin has no agent"
    );

    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();

    // 补偿后应有 agent
    assert_has_one_agent(&pool, user.id).await;
}

#[tokio::test]
async fn bootstrap_idempotent_with_agent() {
    let (pool, _g) = pool().await;
    let email = "bootstrap_idem@example.com";
    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();
    let user = user_repo::find_by_email(&pool, email)
        .await
        .unwrap()
        .unwrap();
    let first_agent = agent_repo::find_by_user(&pool, user.id).await.unwrap();

    // 第二次调用：admin + agent 都已存在 → 无操作
    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();

    let second_agent = agent_repo::find_by_user(&pool, user.id).await.unwrap();
    assert_eq!(
        first_agent.map(|a| a.id),
        second_agent.map(|a| a.id),
        "idempotent: agent unchanged on second call"
    );
    assert_has_one_agent(&pool, user.id).await;
}

#[tokio::test]
async fn bootstrap_idempotent_across_three_calls() {
    let (pool, _g) = pool().await;
    let email = "bootstrap_triple@example.com";
    for _ in 0..3 {
        management::ensure_bootstrap_admin(&pool, &security(email))
            .await
            .unwrap();
    }
    let user = user_repo::find_by_email(&pool, email)
        .await
        .unwrap()
        .unwrap();
    assert_has_one_agent(&pool, user.id).await;
}

// ── CLI admin create（走 management::create，但需密码交互，无法直接测；
//     改测其核心——insert_verified + create_default 的组合） ───────────────

#[tokio::test]
async fn create_default_after_insert_verified_mirrors_register() {
    let (pool, _g) = pool().await;
    let email = "cli_create@example.com";
    // 模拟 CLI create 的核心逻辑：insert_verified(superuser) + create_default
    let dummy_hash = "$2b$10$abcdefghijklmnopqrstuvABCDEFGHIJKLMNOPQRSTUVWXY";
    let user = user_repo::insert_verified(&pool, email, dummy_hash, true)
        .await
        .unwrap();
    let agent = agent_service::create_default(&pool, user.id)
        .await
        .expect("create_default succeeds for fresh admin");
    assert_eq!(agent.user_id, user.id);
    assert_eq!(agent.name, "Home Agent");
    assert_has_one_agent(&pool, user.id).await;
}

// ── promote 补偿（走 management 内部 ensure_agent_for_user 的逻辑） ────────

#[tokio::test]
async fn promote_keeps_agent_for_user_with_agent() {
    let (pool, _g) = pool().await;
    // 普通注册用户已有 agent（register 路径），promote 后 agent 不变
    let (user_id, agent_id) = common::seed_user_and_agent(&pool).await;
    // promote 前：普通用户，已有 agent
    let before = agent_repo::find_by_user(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.id, agent_id);

    // promote（直接调 set_superuser + ensure_agent_for_user 的等价组合）
    user_repo::set_superuser(&pool, user_id, true)
        .await
        .unwrap();
    agent_service::create_default(&pool, user_id)
        .await
        .expect_err("create_default should hit quota (already has agent)");
    // promote 后 agent 不变
    let after = agent_repo::find_by_user(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.id, agent_id, "agent unchanged after promote attempt");
}

#[tokio::test]
async fn promote_compensates_user_without_agent() {
    let (pool, _g) = pool().await;
    // 罕见场景：用户存在但无 agent（理论上 register 总建 agent，防御性测试）
    let dummy_hash = "$2b$10$abcdefghijklmnopqrstuvABCDEFGHIJKLMNOPQRSTUVWXY";
    let user = user_repo::insert_verified(&pool, "edge_no_agent@example.com", dummy_hash, false)
        .await
        .unwrap();
    assert!(
        agent_repo::find_by_user(&pool, user.id)
            .await
            .unwrap()
            .is_none()
    );

    user_repo::set_superuser(&pool, user.id, true)
        .await
        .unwrap();
    // 补偿建 agent
    agent_service::create_default(&pool, user.id)
        .await
        .expect("compensation creates agent");
    assert_has_one_agent(&pool, user.id).await;
}

// ── 配额边界 ────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_agent_quota_enforced() {
    let (pool, _g) = pool().await;
    let email = "quota@example.com";
    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();
    let user = user_repo::find_by_email(&pool, email)
        .await
        .unwrap()
        .unwrap();
    // 已有 1 agent（create_default 建的），再建触发配额
    use axum::response::IntoResponse;
    let err = agent_service::create_default(&pool, user.id)
        .await
        .expect_err("second agent should hit quota");
    let resp = err.into_response();
    assert_eq!(
        resp.status(),
        422,
        "quota exceeded → 422 UNPROCESSABLE_ENTITY"
    );
}

// ── 全链路：admin 补 agent 后唤醒不再 404 ────────────────────────────────

#[tokio::test]
async fn admin_find_by_user_resolves_after_bootstrap() {
    // 验证 admin 补 agent 后，device_service::wake 依赖的 find_by_user 不再返回 None
    // （wake 全链路需要 agent 来派发命令；这是 admin 无法使用服务的根本断点）
    let (pool, _g) = pool().await;
    let email = "wake_chain@example.com";
    management::ensure_bootstrap_admin(&pool, &security(email))
        .await
        .unwrap();
    let user = user_repo::find_by_email(&pool, email)
        .await
        .unwrap()
        .unwrap();
    // 补 agent 前（对照）：seed_user_and_agent 返回的普通用户能 resolve
    // 补 agent 后：admin 也能 resolve
    let agent = agent_repo::find_by_user(&pool, user.id)
        .await
        .unwrap()
        .expect("admin has agent after bootstrap → wake chain resolves");
    assert_eq!(agent.user_id, user.id);
}
