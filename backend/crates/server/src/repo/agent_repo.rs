//! agents 表持久层。

use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::agent::Agent;
use crate::repo::RepoError;

#[derive(sqlx::FromRow)]
struct AgentRow {
    id: i64,
    user_id: i64,
    aid: Uuid,
    name: String,
    pairing_code: String,
    public_key: Option<String>,
    projection_version: i64,
    last_seen: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl From<AgentRow> for Agent {
    fn from(r: AgentRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            aid: r.aid,
            name: r.name,
            pairing_code: r.pairing_code,
            public_key: r.public_key,
            projection_version: r.projection_version,
            last_seen: r.last_seen,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Agent + 关联用户的 is_active（pairing-code 中间件校验用，api-design.md §2.4）。
///
/// `is_active` 不进 `Agent` domain（它不是 agent 的属性，是关联用户的属性），
/// 仅供中间件判断后丢弃。JOIN users 一次性查询，无额外 DB 往返。
#[derive(sqlx::FromRow)]
pub struct AgentWithActive {
    pub id: i64,
    pub user_id: i64,
    aid: Uuid,
    name: String,
    pairing_code: String,
    public_key: Option<String>,
    projection_version: i64,
    last_seen: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    is_active: bool,
}

impl AgentWithActive {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

impl From<AgentWithActive> for Agent {
    fn from(r: AgentWithActive) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            aid: r.aid,
            name: r.name,
            pairing_code: r.pairing_code,
            public_key: r.public_key,
            projection_version: r.projection_version,
            last_seen: r.last_seen,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// 注册时为用户创建默认 agent（1:1 `模型）。pairing_code` 由 service 生成。
pub async fn insert(
    pool: &PgPool,
    user_id: i64,
    aid: Uuid,
    name: &str,
    pairing_code: &str,
) -> Result<Agent, RepoError> {
    let row = sqlx::query_as::<_, AgentRow>(
        r"INSERT INTO agents (user_id, aid, name, pairing_code)
           VALUES ($1, $2, $3, $4)
           RETURNING id, user_id, aid, name, pairing_code, public_key, projection_version,
                     last_seen, created_at, updated_at",
    )
    .bind(user_id)
    .bind(aid)
    .bind(name)
    .bind(pairing_code)
    .fetch_one(pool)
    .await?;
    Ok(Agent::from(row))
}

/// 查用户的默认 agent（GET /agents/default）。
pub async fn find_by_user(pool: &PgPool, user_id: i64) -> Result<Option<Agent>, RepoError> {
    let row = sqlx::query_as::<_, AgentRow>(
        r"SELECT id, user_id, aid, name, pairing_code, public_key, projection_version,
                  last_seen, created_at, updated_at
             FROM agents WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Agent::from))
}

/// 按 `pairing_code` 查 + JOIN users 取 `is_active（SSE` 握手最高频，走 `idx_agents_pairing_code` UNIQUE）。
///
/// JOIN users 校验 is_active（api-design.md §2.4）：禁用用户的 agent 即便 `pairing_code`
/// 仍有效也无法连入。一次性查询，无额外 DB 往返。
pub async fn find_by_pairing_code(
    pool: &PgPool,
    pairing_code: &str,
) -> Result<Option<AgentWithActive>, RepoError> {
    let row = sqlx::query_as::<_, AgentWithActive>(
        r"SELECT a.id, a.user_id, a.aid, a.name, a.pairing_code, a.public_key,
                  a.projection_version, a.last_seen, a.created_at, a.updated_at, u.is_active
             FROM agents a JOIN users u ON a.user_id = u.id
            WHERE a.pairing_code = $1",
    )
    .bind(pairing_code)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 按 id 查。
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Agent>, RepoError> {
    let row = sqlx::query_as::<_, AgentRow>(
        r"SELECT id, user_id, aid, name, pairing_code, public_key, projection_version,
                  last_seen, created_at, updated_at
             FROM agents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Agent::from))
}

/// 写入/更新 `public_key` + `last_seen（SSE` 握手 X-Public-Key 逻辑）。
/// `public_key` 为 Some 时写入（重组标准 SPKI PEM 后存；None 时仅 touch `last_seen`）。
///
/// **device-sync-v3 §8.6 PEM 契约**：agent 发送的是**剥掉 PEM 标记行 + 所有换行后的
/// 纯 base64 体**（HTTP header 不允许裸换行）。server 收到后自己套标准 SPKI PEM 外壳重组：
/// 每 64 字符插换行 + 首尾加 `-----BEGIN/END PUBLIC KEY-----` 标记。
/// 两端严格互逆（agent 去 marker+去换行 → base64 体；server 加换行+加标记 → 标准 PEM）。
/// 重组后的 PEM 必须能被 `RsaPublicKey::from_public_key_pem` 解析。
pub async fn upsert_public_key(
    pool: &PgPool,
    id: i64,
    public_key: Option<&str>,
) -> Result<(), RepoError> {
    if let Some(pk) = public_key {
        let restored = reassemble_spki_pem(pk);
        sqlx::query(
            "UPDATE agents SET public_key = $1, last_seen = now(), updated_at = now() WHERE id = $2",
        )
        .bind(&restored)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query("UPDATE agents SET last_seen = now(), updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// 把 agent 发送的纯 base64 体（剥掉 PEM 标记 + 所有换行）重组为标准 SPKI PEM。
///
/// device-sync-v3 §8.6：输入是一长串连续 base64（无 `-----BEGIN/END-----` 标记、无换行）；
/// 输出按 64 字符折行 + 套标准 SPKI 外壳。
///
/// **防御性**：若输入已含 BEGIN/END 标记（兼容旧 agent 或调试），先剥掉标记再重组，
/// 避免双标记导致 PEM 解析失败。
fn reassemble_spki_pem(base64_body: &str) -> String {
    const BEGIN: &str = "-----BEGIN PUBLIC KEY-----";
    const END: &str = "-----END PUBLIC KEY-----";

    // 剥所有换行与可能残留的 PEM 标记（防御性：兼容已含标记的输入，避免双标记）。
    let mut cleaned: String = base64_body
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r'))
        .collect();
    cleaned = cleaned.replace(BEGIN, "").replace(END, "");
    let cleaned = cleaned.trim();
    let folded = fold_base64(cleaned, 64);
    format!("{BEGIN}\n{folded}\n{END}\n")
}

/// 按 width 折行 base64 文本。
fn fold_base64(s: &str, width: usize) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / width);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && i % width == 0 {
            out.push('\n');
        }
        out.push(ch);
    }
    out
}

/// 更新 `last_seen（命令回报/SSE` 断开时）。
pub async fn touch_last_seen(pool: &PgPool, id: i64) -> Result<(), RepoError> {
    sqlx::query("UPDATE agents SET last_seen = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// **投影版本自增**（device-sync-v3 §5.4：与数据写入同事务）。
///
/// 每次 device/integration 增删改在同一事务内调用：`projection_version += 1` 使推送丢失可检测
/// （agent 上报 ack 后比对，落后则 gap 重推）。返回自增后的新版本。
/// 接受泛型 executor（事务支持）。
pub async fn bump_projection_version(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    id: i64,
) -> Result<i64, RepoError> {
    let row: Option<(i64,)> =
        sqlx::query_as("UPDATE agents SET projection_version = projection_version + 1 WHERE id = $1 RETURNING projection_version")
            .bind(id)
            .fetch_optional(executor)
            .await?;
    Ok(row.map_or(0, |(v,)| v))
}

/// 按 did 查所属 agent_id 并 bump 其 projection_version（admin resync 设备用）。
pub async fn bump_projection_version_by_device(pool: &PgPool, did: Uuid) -> Result<i64, RepoError> {
    let row: Option<(i64,)> = sqlx::query_as(
        "UPDATE agents SET projection_version = projection_version + 1
          WHERE id = (SELECT agent_id FROM devices WHERE did = $1)
          RETURNING projection_version",
    )
    .bind(did)
    .fetch_optional(pool)
    .await?;
    Ok(row.map_or(0, |(v,)| v))
}

/// 轮换 `pairing_code（POST` /agents/default/pairing-code/rotate）。
pub async fn update_pairing_code(pool: &PgPool, id: i64, new_code: &str) -> Result<(), RepoError> {
    sqlx::query("UPDATE agents SET pairing_code = $1, updated_at = now() WHERE id = $2")
        .bind(new_code)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 配额校验：count agents for user（锁定实际行，PG 不允许聚合 + FOR UPDATE）。
pub async fn count_for_update(pool: &PgPool, user_id: i64) -> Result<i64, RepoError> {
    let rows = sqlx::query_as::<_, (i64,)>("SELECT id FROM agents WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.len() as i64)
}

/// 取 `agent_id（admin` 禁用用户时按 `user_id` 找 agent 触发 SSE 断开，1:1 模型）。
pub async fn find_agent_id_by_user(pool: &PgPool, user_id: i64) -> Result<Option<i64>, RepoError> {
    let row = sqlx::query_as::<_, (i64,)>("SELECT id FROM agents WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(id,)| id))
}

#[cfg(test)]
mod pem_tests {
    use super::*;

    #[test]
    fn reassembles_pure_base64_body() {
        // agent 发送：纯 base64 体（无标记、无换行）
        let body = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAukpPFN0nc4WJYVsZIrzGB5BGdBO83hyCrM+dkksPBMxBTY8Zk5LQ+QxNKSgnjsGBhTdzsjjnVCSB+/RRD4pNebMFdqiXYDhsNzZ81CpNgC4jvLxKb0s/7HqrkO5IBo9gRVhcwrx4PDDonmOn7kXoN83PtsrrlMR0D8dj0B7eQf1/adivWu+bI2fM+wCgtWBO7U4PDhFUSO8BZ8ZKnQ9G80prHoBSfld4fAc8ugToe1GnrK5sly9bhOtfnAN93zoIMafO6QHTd4kXChXoHHngJomYU+sJ4nQ/asctSL6hM7eVQbyWg63XlC9cn39MgKz6VgDkf+Em0r12YQeqxgo3jwIDAQAB";
        let restored = reassemble_spki_pem(body);
        assert!(restored.starts_with("-----BEGIN PUBLIC KEY-----\n"));
        assert!(restored.ends_with("\n-----END PUBLIC KEY-----\n"));
        // 折行后每行 ≤64
        for line in restored
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with("-----END"))
        {
            assert!(line.len() <= 64);
        }
        // 提取 body 与原始一致（去换行后）
        let extracted: String = restored
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .flat_map(|l| l.chars())
            .collect();
        assert_eq!(extracted, body);
    }

    #[test]
    fn strips_stray_markers_defensively() {
        // 兼容：输入已含标记（双标记防护）——重组后只应有一对标记
        let with_markers = "-----BEGIN PUBLIC KEY-----MIIBIjANBgk==-----END PUBLIC KEY-----";
        let restored = reassemble_spki_pem(with_markers);
        assert_eq!(
            restored.matches("-----BEGIN PUBLIC KEY-----").count(),
            1,
            "should not double-mark"
        );
        assert_eq!(
            restored.matches("-----END PUBLIC KEY-----").count(),
            1,
            "should not double-mark"
        );
        assert!(restored.contains("MIIBIjANBgk=="));
    }

    #[test]
    fn strips_internal_newlines() {
        // agent 误发已折行的 base64 体（无标记但有换行）——重组应去换行后重折
        let folded = "MIIBIjAN\nBgkqhkiG";
        let restored = reassemble_spki_pem(folded);
        let extracted: String = restored
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .flat_map(|l| l.chars())
            .collect();
        assert_eq!(extracted, "MIIBIjANBgkqhkiG");
    }
}
