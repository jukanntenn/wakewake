//! wakewake-seed：压测数据 DB 直灌（load.md §5）。
//!
//! users + agents + devices（每用户 2 台）+ integrations（比例用户带 bemfa），
//! PG COPY FROM STDIN 批量灌入（比 INSERT 快 10-100 倍）。
//! 复用 `wakewake_server::service::secrets::generate_pairing_code（16` hex 零漂移）。
//!
//! 数据一致性（load.md §5.3）：
//! - `users.is_active` = `true（is_active` 修复后，SSE 鉴权要求 active）。
//! - 所有用户共用固定预计算 bcrypt hash（TestPass123!）。
//! - `pairing_code` 用 `generate_pairing_code()` 复刻（16 hex），内存 set 去重保险。
//!
//! CLI：
//!   cargo run --release -p wakewake-seed -- --count 100000 \
//!     --dsn "<postgres://wakewake:test@localhost:5432/wakewake_test>"

use std::collections::HashSet;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use sqlx::PgPool;
use sqlx::postgres::{PgCopyIn, PgPoolCopyExt, PgPoolOptions};
use time::OffsetDateTime;
use uuid::Uuid;
use wakewake_server::service::secrets::generate_pairing_code;

/// seed CLI（load.md §5.2）。
#[derive(Parser, Debug)]
#[command(name = "wakewake-seed", about = "压测数据 DB 直灌")]
struct Cli {
    /// 灌入行数（users + agents 各 count 行）
    #[arg(long, default_value_t = 100_000)]
    count: usize,
    /// `PostgreSQL` DSN
    #[arg(long)]
    dsn: String,
    /// 固定密码（所有压测用户共用）
    #[arg(long, default_value = "TestPass123!")]
    password: String,
    /// 带 bemfa 集成的用户比例（0 = 不灌 integrations）
    #[arg(long, default_value_t = 0.3)]
    intg_fraction: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing();

    tracing::info!(count = cli.count, "starting seed");

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&cli.dsn)
        .await
        .context("connect PG")?;

    // 预计算固定 bcrypt hash（所有用户共用，load.md §5.3）
    let fixed_hash = bcrypt::hash(cli.password.as_str(), wakewake_server::domain::BCRYPT_COST)
        .context("bcrypt hash")?;
    tracing::info!("fixed bcrypt hash computed");

    let started = Instant::now();
    let now = OffsetDateTime::now_utc();
    let now_rfc = now
        .format(&time::format_description::well_known::Rfc3339)
        .context("format now rfc3339")?;

    // ---- 1. COPY users ----
    // 用 sqlx 0.9 PgPoolCopyExt::copy_in_raw + PgCopyIn::send/finish（load.md §5.4）
    let mut user_copy: PgCopyIn<_> = pool
        .copy_in_raw(
            "COPY users (email, password, is_active, is_superuser, created_at, updated_at) FROM STDIN WITH (FORMAT csv)",
        )
        .await
        .context("COPY users init")?;

    // CSV 字段需处理 bcrypt hash 的 `$` 和 `,` —— bcrypt hash 无逗号，但用 CSV 引号保险
    // email 格式：seed-000001@load.wakewake.local（10 万不重复，唯一索引）
    let mut user_csv = String::with_capacity(cli.count * 80);
    for i in 0..cli.count {
        user_csv.push_str(&format!(
            "seed-{:06}@load.wakewake.local,{},t,f,{},{}\n",
            i,
            csv_quote(&fixed_hash),
            now_rfc,
            now_rfc,
        ));
    }
    user_copy.send(user_csv.as_bytes()).await?;
    let user_rows = user_copy.finish().await?;
    tracing::info!(user_rows, "users COPY done");

    // ---- 2. COPY agents ----
    // pairing_code 用 generate_pairing_code() 复刻（16 hex，load.md §5.3）
    // 内存 set 去重保险（10 万个 set 无压力，load.md §5.5）
    let mut seen_codes: HashSet<String> = HashSet::with_capacity(cli.count);
    let mut agent_csv = String::with_capacity(cli.count * 120);
    // 先查 seed users 的 id 范围（COPY 后 PG 分配的 BIGSERIAL）
    // 简化：假设 users.id 从某个起始连续——用 RETURNING 不可行（COPY 无 RETURNING），
    // 改用 子查询：agent.user_id = (SELECT id FROM users WHERE email = ...)
    // 但 COPY 时无法做子查询，所以 agents 的 user_id 用查询后的实际 id。
    let user_ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email LIKE 'seed-%@load.wakewake.local' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .context("fetch seed user ids")?;

    for (user_id,) in &user_ids {
        // pairing_code 去重（生日碰撞概率极低，但保险）
        let mut code = generate_pairing_code();
        while !seen_codes.insert(code.clone()) {
            code = generate_pairing_code();
        }
        let aid = Uuid::new_v4();
        agent_csv.push_str(&format!(
            "{user_id},{aid},\"Home Agent\",{code},,,{now_rfc},{now_rfc}\n",
        ));
    }
    let mut agent_copy: PgCopyIn<_> = pool
        .copy_in_raw(
            "COPY agents (user_id, aid, name, pairing_code, public_key, last_seen, created_at, updated_at) FROM STDIN WITH (FORMAT csv)",
        )
        .await
        .context("COPY agents init")?;
    agent_copy.send(agent_csv.as_bytes()).await?;
    let agent_rows = agent_copy.finish().await?;
    tracing::info!(agent_rows, "agents COPY done");

    // ---- 2b. COPY devices + integrations ----
    let agent_ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM agents WHERE user_id IN \
         (SELECT id FROM users WHERE email LIKE 'seed-%@load.wakewake.local') ORDER BY user_id",
    )
    .fetch_all(&pool)
    .await
    .context("fetch seed agent ids")?;
    let device_rows = seed_devices(&pool, &agent_ids, &now_rfc).await?;
    tracing::info!(device_rows, "devices COPY done");
    let intg_rows = seed_integrations(&pool, &agent_ids, cli.intg_fraction, &now_rfc).await?;
    tracing::info!(intg_rows, "integrations COPY done");

    let elapsed = started.elapsed();
    tracing::info!(?elapsed, user_rows, agent_rows, "seed complete");

    // ---- 3. 一致性自检（load.md §5.6）----
    consistency_check(&pool).await?;

    Ok(())
}

/// RSA-2048 OAEP 密文的 base64 长度（mac_encrypted / bemfa config.uid 占位口径）。
const CIPHER_B64_LEN: usize = 344;

/// COPY devices：每用户 2 台（MAX_DEVICES_PER_USER 口径）。
/// mac_encrypted 用 344 字符 base64 占位——保证 state snapshot payload /
/// 重连风暴带宽不失真（k6 假 agent 不解密；load.md §5.7）。
async fn seed_devices(pool: &PgPool, agent_ids: &[(i64, i64)], now_rfc: &str) -> Result<u64> {
    let cipher_placeholder = "A".repeat(CIPHER_B64_LEN);
    let mut csv = String::with_capacity(agent_ids.len() * 2 * 480);
    for (i, (agent_id, user_id)) in agent_ids.iter().enumerate() {
        for d in 0..2 {
            let mac = format!(
                "AA:BB:CC:{:02X}:{:02X}:{:02X}",
                (i >> 8) & 0xFF,
                i & 0xFF,
                d
            );
            csv.push_str(&format!(
                "{},{},{},\"PC-{}\",{},{},{},{}\n",
                Uuid::new_v4(),
                user_id,
                agent_id,
                d,
                cipher_placeholder,
                mac,
                now_rfc,
                now_rfc,
            ));
        }
    }
    // 列以线上 schema 为准（后续迁移已去 sync_status，bemfa 观测列均可空默认）
    let mut copy: PgCopyIn<_> = pool
        .copy_in_raw(
            "COPY devices (did, user_id, agent_id, name, mac_encrypted, mac_display, created_at, updated_at) FROM STDIN WITH (FORMAT csv)",
        )
        .await
        .context("COPY devices init")?;
    copy.send(csv.as_bytes()).await?;
    copy.finish().await.context("COPY devices finish")
}

/// COPY integrations：前 fraction 比例用户带 bemfa（config.uid 同为密文占位）。
async fn seed_integrations(
    pool: &PgPool,
    agent_ids: &[(i64, i64)],
    fraction: f64,
    now_rfc: &str,
) -> Result<u64> {
    let n = (fraction * agent_ids.len() as f64) as usize;
    if n == 0 {
        return Ok(0);
    }
    let cipher_placeholder = "A".repeat(CIPHER_B64_LEN);
    let mut csv = String::with_capacity(n * 460);
    for (agent_id, user_id) in agent_ids.iter().take(n) {
        let config = format!("{{\"uid\":\"{cipher_placeholder}\"}}");
        csv.push_str(&format!(
            "{},{agent_id},bemfa,{},t,{},{}\n",
            user_id,
            csv_quote(&config),
            now_rfc,
            now_rfc,
        ));
    }
    let mut copy: PgCopyIn<_> = pool
        .copy_in_raw(
            "COPY integrations (user_id, agent_id, provider, config, enabled, created_at, updated_at) FROM STDIN WITH (FORMAT csv)",
        )
        .await
        .context("COPY integrations init")?;
    copy.send(csv.as_bytes()).await?;
    copy.finish().await.context("COPY integrations finish")
}

/// CSV 字段引号包装（含 `$`/`,` 的值用双引号包裹，内部双引号转义为两个）。
fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 一致性自检（load.md §5.6）。
async fn consistency_check(pool: &PgPool) -> Result<()> {
    let user_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE email LIKE 'seed-%@load.wakewake.local'",
    )
    .fetch_one(pool)
    .await?;
    let bad_codes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agents WHERE pairing_code !~ '^[0-9a-f]{16}$'")
            .fetch_one(pool)
            .await?;
    let orphan_agents: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agents a LEFT JOIN users u ON a.user_id = u.id WHERE u.id IS NULL",
    )
    .fetch_one(pool)
    .await?;
    let dup_codes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) - COUNT(DISTINCT pairing_code) FROM agents")
            .fetch_one(pool)
            .await?;
    let orphan_devices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM devices d LEFT JOIN agents a ON d.agent_id = a.id WHERE a.id IS NULL",
    )
    .fetch_one(pool)
    .await?;
    let bad_mac_cipher: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM devices WHERE length(mac_encrypted) <> 344 OR mac_encrypted !~ '^[A-Za-z0-9+/=]+$'",
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(
        user_count,
        bad_codes,
        orphan_agents,
        dup_codes,
        orphan_devices,
        bad_mac_cipher,
        "consistency check"
    );
    assert!(
        bad_codes == 0,
        "agents with malformed pairing_code: {bad_codes}"
    );
    assert!(
        orphan_agents == 0,
        "orphan agents (FK broken): {orphan_agents}"
    );
    assert!(dup_codes == 0, "duplicate pairing_codes: {dup_codes}");
    assert!(
        orphan_devices == 0,
        "orphan devices (FK broken): {orphan_devices}"
    );
    assert!(
        bad_mac_cipher == 0,
        "devices with malformed mac_encrypted: {bad_mac_cipher}"
    );
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
