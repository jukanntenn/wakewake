//! 处理 agent 上行：complete（wake 命令回报）+ sync（观测/ack）+ wakes（MQTT 触发）（device-sync-v3 §8.4 / §8.6）。
//!
//! device-sync-v3 重构：
//! - `complete`：仅 wake 命令（`c_` 前缀）。`s_` state-ack 占位已废，state ack 改走 `POST /sync` 的 `applied_version`。
//! - `handle_sync`：记录 ack 水位（hub.record_ack）+ 若含 integration 块：set_runtime_status +
//!   逐 observation apply_observation（漂移判定 §9.3）+ 返回 current_version（agent 据此判 I6 守卫）。
//! - `handle_wake_report`：写 wakes(type=bemfa_wake) 审计记录。

use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;
use wakewake_protocol::{CommandPayload, SyncIntegration, SyncRequest, WakeRequest};

use crate::domain::command::{CommandOutcome, prefix};
use crate::domain::wake::{RecordWake, WakeStatus, WakeType};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::hub::CommandDispatcher;
use crate::repo::{agent_repo, device_repo, integration_repo};

/// 唤醒审计写入通道（异步 buffered，不阻塞命令 ack 链路）。
pub type WakeWriter = mpsc::Sender<RecordWake>;

/// 处理 agent 的 complete 回报（仅 wake 命令）。
/// 幂等：command 不存在则忽略（重复 complete 网络重传）。
pub async fn handle_complete<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    wake_writer: Option<&WakeWriter>,
    command_id: &str,
    outcome: CommandOutcome,
) -> AppResult<bool> {
    let command = match dispatcher.get_command(command_id) {
        Some(c) => c,
        None => return Ok(false),
    };

    let success = outcome.success;
    let outcome_message = outcome.message.clone();
    dispatcher.complete(command_id, outcome);

    // device-sync-v3：仅 c_ 前缀（wake 命令）。s_ 已废。
    if command_id.starts_with(prefix::COMMAND) {
        let CommandPayload::Wol { device } = &command.payload;
        {
            let user_id = match agent_user_id(pool, command.agent_id).await {
                Ok(uid) => uid,
                Err(e) => {
                    tracing::warn!(error = ?e, "wol complete: failed to resolve agent user_id, skipping wake record");
                    return Ok(true);
                },
            };
            let device_name = device_name_by_did(pool, &device.did)
                .await
                .unwrap_or_else(|_| device.did.clone());
            if let Some(writer) = wake_writer {
                let _ = writer
                    .try_send(RecordWake {
                        user_id,
                        device_did: parse_did(&device.did),
                        device_name,
                        r#type: WakeType::Wol,
                        status: if success {
                            WakeStatus::Success
                        } else {
                            WakeStatus::Failed
                        },
                        message: (!outcome_message.is_empty()).then_some(outcome_message.clone()),
                    })
                    .ok();
            }
        }
    }
    Ok(true)
}

/// 处理 `POST /agents/self/sync`（device-sync-v3 §8.6 上行 2）。
///
/// 1. 记录 ack 水位（`hub.record_ack(applied_version)`）
/// 2. 若含 integration 块：
///    - `set_runtime_status`（mqtt_connected/last_error/last_report_at，§7.2）
///    - 逐 observation `apply_observation`（§9.3 漂移判定，已删设备静默忽略）
///    - 处理 `repair_errors`（按 did 落 `devices.last_error`，已删设备静默忽略）
/// 3. 返回 `current_version`（= agent.projection_version，agent 据此判 I6 孤儿删除守卫，§10.9）
pub async fn handle_sync<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    agent_id: i64,
    req: SyncRequest,
) -> AppResult<wakewake_protocol::SyncResponse> {
    // 1. 记录 ack 水位
    if let Some(applied) = req.applied_version {
        dispatcher.record_ack(agent_id, i64::try_from(applied).unwrap_or(0));
    }

    // 2. 处理 integration 块（若有）
    if let Some(integration) = &req.integration {
        handle_integration_block(pool, agent_id, integration).await?;
    }

    // 3. 返回 current_version（I6 守卫支撑）
    let agent = agent_repo::find_by_id(pool, agent_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;
    Ok(wakewake_protocol::SyncResponse {
        current_version: u64::try_from(agent.projection_version).unwrap_or(0),
    })
}

/// 处理 sync 的 integration 块：runtime_status + observations + repair_errors。
async fn handle_integration_block(
    pool: &PgPool,
    agent_id: i64,
    integration: &SyncIntegration,
) -> AppResult<()> {
    // set_runtime_status：has_integration_block=true（出现 integration 块即刷 last_report_at，§7.2）
    integration_repo::set_runtime_status(
        pool,
        agent_id,
        &integration.provider,
        integration.mqtt_connected,
        integration.error.as_deref(),
        true,
    )
    .await
    .map_err(AppError::from_repo)?;

    // 逐 observation apply_observation（§9.3，同事务内 SELECT FOR UPDATE + 派生 drift + UPDATE）
    for obs in &integration.observations {
        let did = parse_did(&obs.did);
        // repair_error：按 did 在 repair_errors 中找（每轮覆盖；§9.3 last_error 反映上轮修复）
        let repair_error = integration
            .repair_errors
            .iter()
            .find(|r| parse_did(&r.did) == did)
            .map(|r| r.error.as_str());
        let mut tx = pool.begin().await.map_err(AppError::from)?;
        // 已删设备静默忽略（apply_observation 返 Ok(false)，不报错，§9.3）。
        // apply_observation 接受 &mut PgConnection（Transaction deref 到 PgConnection，用 &mut *tx）。
        let _ = device_repo::apply_observation(&mut tx, did, obs, repair_error)
            .await
            .map_err(AppError::from_repo)?;
        tx.commit().await.map_err(AppError::from)?;
    }

    Ok(())
}

/// 处理 `POST /agents/self/wakes`（MQTT on 触发的语音唤醒上报，§8.6 上行 3）。
///
/// 写 wakes(type=bemfa_wake) 审计记录。device_name 兜底用 did 字符串（§7.0：设备已删时）。
/// 返回新建记录 id。
pub async fn handle_wake_report(
    pool: &PgPool,
    agent_id: i64,
    req: WakeRequest,
) -> AppResult<wakewake_protocol::WakeResponse> {
    let user_id = agent_user_id(pool, agent_id)
        .await
        .map_err(AppError::from)?;
    let device_name = device_name_by_did(pool, &req.did)
        .await
        .unwrap_or_else(|_| req.did.clone());
    let device_did = parse_did(&req.did);

    // 同步写 wakes 记录（201 语义：资源已创建）。
    // status 用字符串字面量（WakeStatus 枚举未派生 sqlx::Type）。
    let status_str = if req.success {
        WakeStatus::Success
    } else {
        WakeStatus::Failed
    }
    .as_str();
    let id: i64 = sqlx::query_scalar(
        r"INSERT INTO wakes (user_id, device_did, device_name, type, status, message)
           VALUES ($1, $2, $3, 'bemfa_wake', $4, $5) RETURNING id",
    )
    .bind(user_id)
    .bind(device_did)
    .bind(&device_name)
    .bind(status_str)
    .bind((!req.message.is_empty()).then_some(req.message.as_str()))
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;

    Ok(wakewake_protocol::WakeResponse { id })
}

/// 查 agent.user_id（wakes.user_id FK 必须正确）。
async fn agent_user_id(pool: &PgPool, agent_id: i64) -> Result<i64, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM agents WHERE id = $1")
        .bind(agent_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map_or(0, |(uid,)| uid))
}

/// 按 did hex 查 device.name（wake 记录的 `device_name` 快照；设备已删兜底用 did 字符串）。
async fn device_name_by_did(pool: &PgPool, did_hex: &str) -> Result<String, sqlx::Error> {
    let did = parse_did(did_hex);
    let row: Option<(String,)> = sqlx::query_as("SELECT name FROM devices WHERE did = $1")
        .bind(did)
        .fetch_optional(pool)
        .await?;
    Ok(row.map_or_else(|| did_hex.to_string(), |(n,)| n))
}

/// 无效命令 id（不匹配 c_ 前缀）。
pub fn validate_command_id(id: &str) -> AppResult<()> {
    if id.starts_with(prefix::COMMAND) {
        Ok(())
    } else {
        Err(AppError::code(ErrorCode::DeviceNotFound))
    }
}

fn parse_did(hex: &str) -> Uuid {
    Uuid::parse_str(hex).unwrap_or_else(|_| Uuid::nil())
}
