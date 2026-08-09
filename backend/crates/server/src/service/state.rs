//! State 快照构建 + 投影推送（device-sync-v3 §5.4 / §8.6）。
//!
//! device-sync-v3 重构：state 快照携带单调 `version`（= `agents.projection_version`）+ `aid`
//! （= `agents.aid` 的 simple 形式，agent 据此派生 topic 命名 k4）。
//! `refresh_for_agent` 是 DB 写操作后主动推送 state 的统一入口：bump version（与数据写同事务，
//! §5.4）→ 构建快照 → hub.push_state。推送丢失由 5s gap 重推自愈。

use sqlx::PgPool;
use wakewake_protocol::{DeviceData, IntegrationData, StateSnapshot};

use crate::error::{AppError, AppResult};
use crate::hub::CommandDispatcher;
use crate::repo::{agent_repo, device_repo, integration_repo};

/// 为 agent 构建全量 state 快照（含 version + aid，§8.6）。
///
/// - `version` = `agents.projection_version`
/// - `aid` = `agents.aid` 的 `Uuid::simple()`（32-hex）
pub async fn build_for_agent(pool: &PgPool, agent_id: i64) -> AppResult<StateSnapshot> {
    let agent = agent_repo::find_by_id(pool, agent_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or_else(|| AppError::code(crate::error::ErrorCode::AgentNotFound))?;
    let devices = device_repo::list_by_agent(pool, agent_id)
        .await
        .map_err(AppError::from_repo)?;
    let integrations = integration_repo::list_by_agent(pool, agent_id)
        .await
        .map_err(AppError::from_repo)?;

    Ok(StateSnapshot {
        version: u64::try_from(agent.projection_version).unwrap_or(0),
        aid: agent.aid.simple().to_string(),
        devices: devices
            .into_iter()
            .map(|d| DeviceData {
                did: d.did.simple().to_string(),
                name: d.name,
                mac_encrypted: d.mac_encrypted,
                description: d.description,
            })
            .collect(),
        integrations: integrations
            .into_iter()
            .map(|i| IntegrationData {
                provider: i.provider,
                config: i.config,
                enabled: i.enabled,
            })
            .collect(),
    })
}

/// DB 写操作后主动推送一份新的全量 state 快照给 agent（投影通道，§5.4）。
///
/// 构建快照（含当前 projection_version）→ `hub.push_state`（在线则经 SSE `event: state` 下发；
/// 离线则忽略——agent 重连时 SSE 握手会推全量）。失败（agent 离线）不视为错误：
/// 仅记 warn，因为 agent 重连 + 5s gap 重推会自然追平。
///
/// **注意**：调用方必须已在同事务内 bump `projection_version`（§5.4：版本自增与数据写入同事务，
/// 否则崩溃窗口出现"数据已变、版本未变" → gap 永不被检测）。本函数只读当前 version。
pub async fn refresh_for_agent<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    agent_id: i64,
) -> AppResult<()> {
    let snapshot = build_for_agent(pool, agent_id).await?;
    let version = i64::try_from(snapshot.version).unwrap_or(0);
    if let Err(e) = dispatcher.push_state(agent_id, snapshot, version) {
        tracing::warn!(agent_id, error = %e, "state refresh: agent offline, skipped (gap repush will heal)");
    }
    Ok(())
}
