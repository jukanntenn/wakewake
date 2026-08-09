//! 数据库 pool 创建 + migration 执行（server 与 management CLI 共用）。
//!
//! 从 `main.rs` 提取，使 [`crate::management`] 子命令能复用同一套连接 + 迁移逻辑，
//! 避免在两处重复 `PgPoolOptions`/`migrate!` 调用。

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// 创建连接池。`max_connections=20` 对齐 PG `max_connections`（perf-est.md §10.3）。
pub async fn create_pool(dsn: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().max_connections(20).connect(dsn).await
}

/// 应用编译期嵌入的迁移（`sqlx::migrate!`，NNNNNN_*.up.sql 自动执行）。
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}
