//! 风控运行时控制集成测试（admin-risk-controls WRFC）。
//! 验证未验证账号清理 SQL 守卫 + 风控聚合查询正确性（真实 PG）。

mod common;

use sqlx::PgPool;
use wakewake_server::repo::{admin_repo, user_repo};

/// 造一个用户，返回 id。verified/created_at 可控。
async fn seed_user(
    pool: &PgPool,
    email: &str,
    verified: bool,
    created_days_ago: i32,
    su: bool,
) -> i64 {
    let hash = bcrypt::hash("pass1234", 4).unwrap();
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO users (email, password, email_verified, is_superuser, created_at)
         VALUES ($1, $2, $3, $4, now() - make_interval(days => $5)) RETURNING id",
    )
    .bind(email)
    .bind(&hash)
    .bind(verified)
    .bind(su)
    .bind(created_days_ago)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn purge_deletes_only_stale_unverified_non_superuser() {
    let (pool, _g) = common::pool_locked().await;

    let stale = seed_user(
        &pool,
        &format!("purge_stale_{}@x.com", uuid::Uuid::new_v4()),
        false,
        30,
        false,
    )
    .await;
    let fresh = seed_user(
        &pool,
        &format!("purge_fresh_{}@x.com", uuid::Uuid::new_v4()),
        false,
        1,
        false,
    )
    .await;
    let verified_old = seed_user(
        &pool,
        &format!("purge_vold_{}@x.com", uuid::Uuid::new_v4()),
        true,
        90,
        false,
    )
    .await;
    let su_unverified = seed_user(
        &pool,
        &format!("purge_su_{}@x.com", uuid::Uuid::new_v4()),
        false,
        90,
        true,
    )
    .await;

    // stale 用户附带 agent 行（注册流程同构）——级联删除验证
    sqlx::query("INSERT INTO agents (user_id, aid, name, pairing_code) VALUES ($1, $2, 'a', $3)")
        .bind(stale)
        .bind(uuid::Uuid::new_v4())
        .bind(&uuid::Uuid::new_v4().simple().to_string()[..16])
        .execute(&pool)
        .await
        .unwrap();

    let purged = user_repo::purge_unverified(&pool, 7).await.unwrap();
    assert!(purged >= 1, "at least the stale account is purged");

    async fn exists(pool: &PgPool, id: i64) -> bool {
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        n == 1
    }
    assert!(!exists(&pool, stale).await, "stale unverified purged");
    assert!(exists(&pool, fresh).await, "within retention kept");
    assert!(exists(&pool, verified_old).await, "verified never purged");
    assert!(exists(&pool, su_unverified).await, "superuser never purged");

    // agents 级联删除
    let (agents,): (i64,) = sqlx::query_as("SELECT count(*) FROM agents WHERE user_id = $1")
        .bind(stale)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(agents, 0, "agent rows cascade with purged user");
}

#[tokio::test]
async fn risk_aggregates_counts_and_top_lists() {
    let (pool, _g) = common::pool_locked().await;

    // 注册：1 个 24h 内 + 1 个 7 天前
    let _new = seed_user(
        &pool,
        &format!("risk_new_{}@x.com", uuid::Uuid::new_v4()),
        false,
        0,
        false,
    )
    .await;
    let _old = seed_user(
        &pool,
        &format!("risk_old_{}@x.com", uuid::Uuid::new_v4()),
        false,
        8,
        false,
    )
    .await;

    // 失败登录：attacker IP 打两个邮箱，另一 IP 打一个
    let a1 = format!("risk_v1_{}@x.com", uuid::Uuid::new_v4());
    let a2 = format!("risk_v2_{}@x.com", uuid::Uuid::new_v4());
    for email in [&a1, &a1, &a2] {
        sqlx::query("INSERT INTO login_events (email, success, ip_address, failure_code) VALUES ($1, false, '203.0.113.50'::inet, 'INVALID_CREDENTIALS')")
            .bind(email)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO login_events (email, success, ip_address, failure_code) VALUES ($1, false, '198.51.100.9'::inet, 'INVALID_CREDENTIALS')")
        .bind(&a2)
        .execute(&pool)
        .await
        .unwrap();
    // 成功登录不计入失败聚合
    sqlx::query("INSERT INTO login_events (email, success, ip_address) VALUES ($1, true, '203.0.113.50'::inet)")
        .bind(&a1)
        .execute(&pool)
        .await
        .unwrap();

    let agg = admin_repo::risk_aggregates(&pool).await.unwrap();

    assert!(agg.registrations_24h >= 1);
    assert!(agg.registrations_7d >= agg.registrations_24h);
    assert!(agg.unverified_count >= 2);
    assert!(agg.failed_logins_24h >= 4);
    assert!(
        agg.oldest_unverified_age_hours.unwrap_or(0) >= 24 * 7,
        "8-day-old unverified seeds"
    );

    let top_ip = agg
        .top_failed_ips
        .iter()
        .find(|r| r.ip == "203.0.113.50")
        .expect("attacker ip in top list");
    assert_eq!(top_ip.failures, 3);
    assert_eq!(top_ip.distinct_emails, 2, "sprayed two target emails");

    assert!(
        agg.top_failed_emails
            .iter()
            .any(|r| r.email == a2 && r.distinct_ips == 2),
        "target hit from two ips"
    );
}
