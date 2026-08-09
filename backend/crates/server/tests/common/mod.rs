//! 测试公共工具：建立到测试 PG 的连接池 + 迁移。
//!
//! testing/strategy.md §2：DB 不 mock，用真实 PG 测试库。
// 本模块被多个 test binary 共享，单个 binary 可能不用全部 helper，允许 dead_code。
#![allow(dead_code)]

use sqlx::Connection;
use sqlx::PgPool;
use sqlx::postgres::{PgConnection, PgPoolOptions};

/// 测试隔离锁的 token（pg_advisory_lock 的 64-bit key，全局固定值）。
/// 所有共享测试库的集成测试用同一把锁串行化（跨 thread + 跨 test binary / 进程）。
const TEST_ADVISORY_LOCK_KEY: i64 = 8_817_273; // 固定魔数（不冲突即可）

/// 取 PG advisory lock + 建池 + 清表。
///
/// `clean_all` 删全表，并发测试（同 binary 多线程 + 跨 binary 多进程）会互相删数据
/// 导致 FK 竞态。用 PG `pg_advisory_lock`（session-tied）串行化：一条**独立连接**
/// （不经 pool，Drop 即关 session → 锁自动释放）持锁到测试结束。
/// 跨进程安全（同一 PG 实例），无需额外依赖。
///
/// 返回 (pool, guard)——guard 必须活到测试结束以持锁（Drop 关连接 → session 结束 → 锁释放）。
pub async fn pool_locked() -> (PgPool, AdvisoryLockGuard) {
    let pool = setup_pool().await;
    // 独立连接（非 pool 连接）：持锁期间不被复用，Drop 直接关 session 释放锁。
    let mut lock_conn = PgConnection::connect(&database_url())
        .await
        .expect("connect lock conn");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(TEST_ADVISORY_LOCK_KEY)
        .execute(&mut lock_conn)
        .await
        .expect("pg_advisory_lock");
    clean_all(&pool).await;
    (pool, AdvisoryLockGuard { conn: lock_conn })
}

/// 持有 advisory lock 的独立连接 guard。Drop 时连接关闭 → session 结束 → 锁自动释放。
pub struct AdvisoryLockGuard {
    #[allow(dead_code)]
    conn: PgConnection,
}

/// 测试用 `DATABASE_URL（默认` `wakewake_dev；可被` env 覆盖）。
pub fn database_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://wakewake:devpass@localhost:5432/wakewake_dev".into())
}

/// 建池 + 跑迁移。每个集成测试文件调一次。
pub async fn setup_pool() -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url())
        .await
        .expect("connect to test PG");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

/// 清空所有业务表（测试隔离，按外键依赖顺序）。
pub async fn clean_all(pool: &PgPool) {
    // 顺序：先删有 FK 依赖的，最后删 users。
    // admin_actions 引用 users（RESTRICT），必须先于 users 删除。
    sqlx::query("DELETE FROM admin_actions")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM wakes").execute(pool).await.ok();
    sqlx::query("DELETE FROM refresh_tokens")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM integrations")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM devices").execute(pool).await.ok();
    sqlx::query("DELETE FROM agents").execute(pool).await.ok();
    sqlx::query("DELETE FROM users").execute(pool).await.ok();
}

/// 插入测试用户 + 默认 agent，返回 (`user_id`, `agent_id`)。
pub async fn seed_user_and_agent(pool: &PgPool) -> (i64, i64) {
    // bcrypt hash 固定 60 字节：$2b$10$ + 22 salt + 31 hash。
    let dummy_hash = "$2b$10$abcdefghijklmnopqrstuvABCDEFGHIJKLMNOPQRSTUVWXY";
    let user: (i64,) =
        sqlx::query_as("INSERT INTO users (email, password) VALUES ($1, $2) RETURNING id")
            .bind(format!(
                "test_{}@example.com",
                uuid::Uuid::new_v4().simple()
            ))
            .bind(dummy_hash)
            .fetch_one(pool)
            .await
            .expect("seed user");
    let agent: (i64,) = sqlx::query_as(
        "INSERT INTO agents (user_id, aid, name, pairing_code) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user.0)
    .bind(uuid::Uuid::new_v4())
    .bind("Home Agent")
    .bind(format!("{:016x}", uuid::Uuid::new_v4().as_u128() & 0xFFFF_FFFF_FFFF_FFFF))
    .fetch_one(pool)
    .await
    .expect("seed agent");
    (user.0, agent.0)
}
