//! 命令执行器：处理 server 下发的命令。
//!
//! device-sync-v3 后 command 通道只承载 `Wol`（唤醒）。设备/集成的增删改全部由
//! `event: state` 全量快照驱动（agent 对账，见 `bemfa_state`）。

use rsa::RsaPrivateKey;
use std::net::SocketAddr;
use wakewake_protocol::{CommandPayload, CommandResult};

use crate::config::WolSettings;
use crate::state::SharedState;
use crate::wol;

#[cfg(test)]
use crate::state::AgentState;

/// 命令执行结果（complete 端点的 body）。
pub async fn execute(
    payload: &CommandPayload,
    private_key: &RsaPrivateKey,
    _state: &SharedState,
    wol_settings: &WolSettings,
) -> CommandResult {
    let CommandPayload::Wol { device } = payload;
    execute_wol(device, private_key, wol_settings).await
}

/// 执行 wol 命令：RSA 解密 MAC → 校验 → 发 magic packet（agents/wol.md §5）。
///
/// 复用 `wol::send_wol`（与巴法语音触发共用同一套解密+发包逻辑）。
async fn execute_wol(
    device: &wakewake_protocol::WolDevice,
    private_key: &RsaPrivateKey,
    wol_settings: &WolSettings,
) -> CommandResult {
    let addr: SocketAddr = wol_settings.broadcast_socket_addr().unwrap_or_else(|_| {
        "255.255.255.255:9"
            .parse()
            .unwrap_or(([255, 255, 255, 255], 9).into())
    });
    match wol::send_wol(
        &device.mac_encrypted,
        private_key,
        addr,
        wol_settings.packet_count,
        std::time::Duration::from_millis(wol_settings.packet_delay_ms),
    )
    .await
    {
        Ok(msg) => CommandResult {
            success: true,
            message: msg,
        },
        Err(msg) => CommandResult {
            success: false,
            message: msg,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wakewake_protocol::WolDevice;

    fn gen_key() -> RsaPrivateKey {
        let mut rng = rand::rng();
        RsaPrivateKey::new(&mut rng, 2048).expect("keygen")
    }

    #[tokio::test]
    async fn wol_with_invalid_ciphertext_fails_gracefully() {
        let state = AgentState::new_shared();
        let key = gen_key();
        let wol = WolSettings::default();
        let payload = CommandPayload::Wol {
            device: WolDevice {
                did: "x".into(),
                mac_encrypted: "not-valid-base64-ct".into(),
            },
        };
        let result = execute(&payload, &key, &state, &wol).await;
        assert!(!result.success);
        assert!(result.message.contains("RSA decryption failed"));
    }

    #[tokio::test]
    async fn state_variant_is_noop() {
        // device-sync-v3：CommandPayload 只有 Wol 变体，无 State。此测试已废弃并移除。
        // 保留函数名为占位避免模块空警告（实际无逻辑）。
    }
}
