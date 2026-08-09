//! Device 服务：CRUD + 配额 + 唤醒派发（device-sync-v3 §5.4 / §8.3）。
//!
//! device-sync-v3：create/update/delete 在**同事务**内 bump `projection_version`（§5.4：
//! 版本自增与数据写入同事务，否则崩溃窗口出现"数据已变、版本未变" → gap 永不被检测）。
//! 写后调 `state::refresh_for_agent` 推全量快照（投影通道）。wake 仍是唯一走 command 通道的。

use sqlx::PgPool;
use uuid::Uuid;
use wakewake_protocol::CommandPayload;

use crate::domain::command::prefix;
use crate::domain::device::{CreateDeviceInput, Device, UpdateDeviceInput};
use crate::error::{AppError, AppResult, ErrorCode, FieldError};
use crate::hub::{self, CommandDispatcher};
use crate::repo::{agent_repo, device_repo};
use crate::service::state;

/// 脱敏 MAC 格式：AA:**:**:**:**:FF。RSA-2048 OAEP(SHA-256) 密文 base64 长度下限（实测 344，留余量取 256）。
const MAC_ENCRYPTED_MIN_LEN: usize = 256;

/// 校验 `mac_display` 是否为脱敏格式 `XX:**:**:**:**:XX`。
fn is_valid_masked_mac(s: &str) -> bool {
    let segs: Vec<&str> = s.split(':').collect();
    segs.len() == 6
        && segs[1..5].iter().all(|s| *s == "**")
        && is_hex_byte(segs[0])
        && is_hex_byte(segs[5])
}

/// 2 字符 hex（MAC 单字节）。
fn is_hex_byte(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// 校验 MAC 字段（BUG#3 修复）。
fn validate_mac(mac_encrypted: &str, mac_display: &str) -> AppResult<()> {
    let mut errs = Vec::new();
    if !is_valid_masked_mac(mac_display) {
        errs.push(FieldError::new("mac_display", "invalid_format"));
    }
    if mac_encrypted.len() < MAC_ENCRYPTED_MIN_LEN {
        errs.push(FieldError::new("mac_encrypted", "invalid_format"));
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(errs))
    }
}

/// 列用户设备。
pub async fn list(pool: &PgPool, user_id: i64) -> AppResult<Vec<Device>> {
    device_repo::list_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)
}

/// 创建设备：配额校验（≤2，FOR UPDATE）→ 入库 → 同事务 bump projection_version → 推 state refresh。
pub async fn create<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    input: &CreateDeviceInput,
) -> AppResult<Device> {
    use crate::domain::MAX_DEVICES_PER_USER;
    validate_mac(&input.mac_encrypted, &input.mac_display)?;
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    // 同事务：配额锁 → count → insert device → bump version（§5.4）
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let count = device_repo::count_for_update(&mut *tx, user_id)
        .await
        .map_err(AppError::from_repo)?;
    if count >= MAX_DEVICES_PER_USER {
        return Err(AppError::code(ErrorCode::QuotaExceeded));
    }
    let device = device_repo::insert(
        &mut *tx,
        &device_repo::NewDevice {
            did: Uuid::new_v4(),
            user_id,
            agent_id: agent.id,
            name: &input.name,
            mac_encrypted: &input.mac_encrypted,
            mac_display: &input.mac_display,
            description: input.description.as_deref(),
        },
    )
    .await
    .map_err(AppError::from_repo)?;
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    // 推 state refresh（投影通道，version 已 bump，agent diff 后建巴法云 topic）。
    state::refresh_for_agent(pool, dispatcher, agent.id).await?;
    tracing::info!(
        user_id,
        agent_id = agent.id,
        did = %device.did,
        name = %device.name,
        "device created + projection version bumped + state refresh pushed"
    );
    Ok(device)
}

/// 获取设备（跨用户返 404 防枚举）。
pub async fn get(pool: &PgPool, user_id: i64, did: Uuid) -> AppResult<Device> {
    device_repo::find_by_did_for_user(pool, did, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::DeviceNotFound))
}

/// 更新设备（PATCH）。同事务 bump projection_version → 推 state refresh。
///
/// device-sync-v3 §8.3：mac_encrypted 与 mac_display 必须成对出现（改 MAC 时），否则 VALIDATION_FAILED。
pub async fn update<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    did: Uuid,
    input: &UpdateDeviceInput,
) -> AppResult<Device> {
    // MAC 必须成对出现（§8.3）
    match (&input.mac_encrypted, &input.mac_display) {
        (Some(enc), Some(disp)) => validate_mac(enc, disp)?,
        (Some(_), None) | (None, Some(_)) => {
            return Err(AppError::validation(vec![FieldError::new(
                "mac_encrypted",
                "invalid_format",
            )]));
        },
        (None, None) => {},
    }

    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    // 同事务：update device → bump version（§5.4）
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let device = device_repo::update(
        &mut *tx,
        did,
        user_id,
        input.name.as_deref(),
        input.mac_encrypted.as_deref(),
        input.mac_display.as_deref(),
        input.description.as_ref().map(|opt| opt.as_deref()),
    )
    .await
    .map_err(AppError::from_repo)?
    .ok_or(AppError::code(ErrorCode::DeviceNotFound))?;
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    state::refresh_for_agent(pool, dispatcher, agent.id).await?;
    Ok(device)
}

/// 删除设备：同事务 delete + bump version → 推 state refresh。
pub async fn delete<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    did: Uuid,
) -> AppResult<()> {
    let device = get(pool, user_id, did).await?;
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let deleted = device_repo::delete(&mut *tx, did, user_id)
        .await
        .map_err(AppError::from_repo)?;
    if !deleted {
        return Err(AppError::code(ErrorCode::DeviceNotFound));
    }
    let _ = agent_repo::bump_projection_version(&mut *tx, agent.id)
        .await
        .map_err(AppError::from_repo)?;
    tx.commit().await.map_err(AppError::from)?;

    let _ = device;
    state::refresh_for_agent(pool, dispatcher, agent.id).await?;
    Ok(())
}

/// 唤醒：派 wol 命令。agent 离线返 AGENT_OFFLINE（命令将 60s expired）。
pub async fn wake<D: CommandDispatcher>(
    pool: &PgPool,
    dispatcher: &D,
    user_id: i64,
    did: Uuid,
) -> AppResult<String> {
    let device = get(pool, user_id, did).await?;
    let agent = agent_repo::find_by_user(pool, user_id)
        .await
        .map_err(AppError::from_repo)?
        .ok_or(AppError::code(ErrorCode::AgentNotFound))?;

    let command_id = hub::generate_command_id(prefix::COMMAND);
    let command = hub::new_command(
        command_id.clone(),
        agent.id,
        CommandPayload::Wol {
            device: wakewake_protocol::WolDevice {
                did: device.did.simple().to_string(),
                mac_encrypted: device.mac_encrypted.clone(),
            },
        },
    );
    match dispatcher.dispatch(agent.id, command.clone()) {
        Ok(()) => {
            tracing::info!(
                agent_id = agent.id,
                command_id = %command_id,
                device_name = %device.name,
                "wake command dispatched to online agent"
            );
            crate::observability::metrics::record_command_dispatched();
        },
        Err(crate::hub::DispatcherError::AgentOffline) => {
            tracing::warn!(
                agent_id = agent.id,
                command_id = %command_id,
                device_name = %device.name,
                "wake dispatch: agent offline (command will expire in 60s)"
            );
            dispatcher.store_offline_command(command);
        },
    }
    Ok(command_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_validation_accepts_valid() {
        let enc = "A".repeat(300);
        assert!(validate_mac(&enc, "AA:**:**:**:**:FF").is_ok());
        assert!(validate_mac(&enc, "01:**:**:**:**:ab").is_ok());
    }

    #[test]
    fn mac_validation_rejects_bad_display() {
        let enc = "A".repeat(300);
        assert!(validate_mac(&enc, "AA:BB:CC:DD:EE:FF").is_err());
        assert!(validate_mac(&enc, "invalid-mac").is_err());
        assert!(validate_mac(&enc, "AA:XX:**:**:**:FF").is_err());
        assert!(validate_mac(&enc, "AA:**:**:**:**:GG").is_err());
    }

    #[test]
    fn mac_validation_rejects_short_encrypted() {
        assert!(validate_mac("garbage", "AA:**:**:**:**:FF").is_err());
    }
}
