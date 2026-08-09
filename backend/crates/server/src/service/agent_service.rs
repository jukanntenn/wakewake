//! Agent 服务：状态/pairing/rotate（api-design.md §Agent）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::agent::{Agent, AgentStatus};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::repo::agent_repo;
use crate::service::secrets::generate_pairing_code;

/// 创建用户的默认 agent（1:1 `模型，注册时调用）。配额校验（MAX_AGENTS_PER_USER=1`）。
pub async fn create_default(pool: &PgPool, user_id: i64) -> AppResult<Agent> {
    use crate::domain::MAX_AGENTS_PER_USER;
    let count = agent_repo::count_for_update(pool, user_id)
        .await
        .map_err(AppError::from_repo)?;
    if count >= MAX_AGENTS_PER_USER {
        return Err(AppError::code(ErrorCode::QuotaExceeded));
    }
    let agent = agent_repo::insert(
        pool,
        user_id,
        Uuid::new_v4(),
        "Home Agent",
        &generate_pairing_code(),
    )
    .await
    .map_err(AppError::from_repo)?;
    Ok(agent)
}

/// 获取用户的默认 agent。
pub async fn get_default(pool: &PgPool, user_id: i64) -> AppResult<Agent> {
    agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))
}

/// 推导 agent 在线状态（结合 DB `public_key` + Hub 是否在线）。
#[must_use]
pub fn derive_status(agent: &Agent, sse_connected: bool) -> AgentStatus {
    AgentStatus::classify(agent.public_key.is_some(), sse_connected)
}

/// `pairing_code` 脱敏：pending 返完整值；online/offline 返脱敏值（a1b2****...****0418）。
#[must_use]
pub fn mask_pairing_code(code: &str, status: AgentStatus) -> String {
    match status {
        AgentStatus::Pending => code.to_string(),
        _ => mask_code(code),
    }
}

/// 脱敏：保留前 4 + 后 4，中间用 **** 替代。
fn mask_code(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    if chars.len() <= 8 {
        return "****".to_string();
    }
    let prefix: String = chars[..4].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}****...****{suffix}")
}

/// 轮换 pairing code（旧码立即失效）。
pub async fn rotate_pairing_code(pool: &PgPool, user_id: i64) -> AppResult<Agent> {
    let agent = get_default(pool, user_id).await?;
    let new_code = generate_pairing_code();
    agent_repo::update_pairing_code(pool, agent.id, &new_code)
        .await
        .map_err(AppError::from_repo)?;
    let updated = agent_repo::find_by_id(pool, agent.id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_preserves_ends() {
        let masked = mask_code("a1b2c3d4e5f60718");
        assert_eq!(masked, "a1b2****...****0718");
    }

    #[test]
    fn derive_status_classification() {
        let mut agent = Agent {
            id: 1,
            user_id: 1,
            aid: Uuid::new_v4(),
            name: "x".into(),
            pairing_code: "abcd".into(),
            public_key: None,
            projection_version: 0,
            last_seen: None,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
        };
        assert_eq!(derive_status(&agent, false), AgentStatus::Pending);
        assert_eq!(derive_status(&agent, true), AgentStatus::Pending);

        agent.public_key = Some("KEY".into());
        assert_eq!(derive_status(&agent, false), AgentStatus::Offline);
        assert_eq!(derive_status(&agent, true), AgentStatus::Online);
    }
}
