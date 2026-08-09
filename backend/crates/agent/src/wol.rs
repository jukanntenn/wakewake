//! Magic Packet 构造与发送（agents/wol.md §4）。自研，原生 async。
//!
//! 固定 102 字节 UDP 广播包：6×0xFF + 16×MAC。
//! 默认 255.255.255.255:9，3 包/50ms 间隔（防丢包，借鉴 wakezilla）。

use std::net::SocketAddr;
use std::time::Duration;

use rsa::RsaPrivateKey;
use tokio::net::UdpSocket;

use crate::crypto;
use crate::mac::parse_mac;

const SYNC_STREAM: [u8; 6] = [0xFF; 6];
const MAC_REPEAT: usize = 16;
const PACKET_SIZE: usize = 6 + MAC_REPEAT * 6; // = 102

/// `WoL` 错误（agents/wol.md §6）。绝不 panic——单条失败不影响 agent 进程。
#[derive(thiserror::Error, Debug)]
pub enum WolError {
    #[error("invalid MAC address: {0}")]
    InvalidMac(String),
    #[error("UDP bind/send failed")]
    Io(#[from] std::io::Error),
}

/// 构造 magic packet（栈上固定数组，零堆分配）。
#[must_use]
pub fn magic_packet(mac: &[u8; 6]) -> [u8; PACKET_SIZE] {
    let mut packet = [0u8; PACKET_SIZE];
    packet[..6].copy_from_slice(&SYNC_STREAM);
    for i in 0..MAC_REPEAT {
        packet[6 + i * 6..6 + (i + 1) * 6].copy_from_slice(mac);
    }
    packet
}

/// 发送 magic packet：多包冗余（默认 3 包/50ms）。
/// socket 循环外创建复用；每次新建保证无状态隔离（WoL 极低频）。
pub async fn send_with_retry(mac: &[u8; 6], addr: SocketAddr) -> Result<(), WolError> {
    send_with_retry_count(mac, addr, 3, Duration::from_millis(50)).await
}

/// `配置化发送（packet_count` / `packet_delay` 由 agent 配置控制）。
pub async fn send_with_retry_count(
    mac: &[u8; 6],
    addr: SocketAddr,
    packet_count: u32,
    packet_delay: Duration,
) -> Result<(), WolError> {
    let packet = magic_packet(mac);
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    // 排障关键：记录实际选用的本地源 IP。受限广播（255.255.255.255）只在本机二层
    // 网段泛洪，跨网段场景下源 IP 揭示了 agent 所在网络（如 WSL2 NAT 172.26.x.x），
    // 而 success:true 仅代表 send_to 系统调用返回 Ok，不代表包到达目标网卡。
    let local_addr = socket
        .local_addr()
        .map_or_else(|_| "unknown".into(), |a| a.to_string());
    tracing::info!(
        event = "wol_packet_sent",
        target_addr = %addr,
        source_ip = %local_addr,
        packet_count,
        packet_size = PACKET_SIZE,
        "magic packet dispatched"
    );
    for i in 0..packet_count {
        if i > 0 {
            tokio::time::sleep(packet_delay).await;
        }
        socket.send_to(&packet, addr).await?;
    }
    Ok(())
}

/// 从 MAC 字符串构造并发送（命令处理流程用，agents/wol.md §5）。
pub async fn send_to_mac(mac_str: &str, addr: SocketAddr) -> Result<(), WolError> {
    let mac = parse_mac(mac_str).map_err(|e| WolError::InvalidMac(e.to_string()))?;
    send_with_retry(&mac, addr).await
}

/// 高层共用 WoL：RSA 解密 MAC 密文 → parse MAC → 发 magic packet。
///
/// 手动 `wol` 命令（`command_executor`）与巴法语音触发（`bemfa`）共用此函数，
/// 消除两份解密+发包逻辑分叉。失败返字符串 message（调用方填入 CommandResult/BemfaWake）。
pub async fn send_wol(
    mac_encrypted_base64: &str,
    private_key: &RsaPrivateKey,
    addr: SocketAddr,
    packet_count: u32,
    packet_delay: Duration,
) -> Result<String, String> {
    let mac_str = crypto::decrypt(private_key, mac_encrypted_base64)
        .map_err(|e| format!("RSA decryption failed: {e}"))?;
    let mac = parse_mac(&mac_str).map_err(|e| format!("invalid MAC format: {e}"))?;
    tracing::info!(
        event = "wol_decrypted",
        target_mac_masked = %mask_mac(&mac_str),
        target_addr = %addr,
        "MAC decrypted, sending magic packet"
    );
    send_with_retry_count(&mac, addr, packet_count, packet_delay)
        .await
        .map_err(|e| format!("WoL send failed: {e}"))?;
    Ok(format!("magic packet sent to {addr}"))
}

/// MAC 脱敏：首 2 段 + 尾 2 段明文，中间 4 段替换 `**`（与 server `mac_display` 一致）。
/// 日志/审计可安全输出，不泄露完整 MAC。如 `2C:**:**:**:**:0D`。
#[must_use]
pub fn mask_mac(mac_str: &str) -> String {
    let parts: Vec<&str> = mac_str.split([':', '-']).collect();
    if parts.len() != 6 {
        return "INVALID".into();
    }
    format!(
        "{}:**:**:**:**:{}",
        parts[0].to_uppercase(),
        parts[5].to_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_mac_formats_correctly() {
        assert_eq!(mask_mac("2C:9C:58:8E:DB:0D"), "2C:**:**:**:**:0D");
        assert_eq!(mask_mac("aa:bb:cc:dd:ee:ff"), "AA:**:**:**:**:FF");
        assert_eq!(mask_mac("not-a-mac"), "INVALID");
    }

    #[test]
    fn magic_packet_structure() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let packet = magic_packet(&mac);
        assert_eq!(packet.len(), PACKET_SIZE);
        // 前 6 字节 FF
        assert!(packet[..6].iter().all(|&b| b == 0xFF));
        // 后续 16 段等于 MAC
        for i in 0..MAC_REPEAT {
            assert_eq!(&packet[6 + i * 6..6 + (i + 1) * 6], &mac);
        }
    }

    #[tokio::test]
    async fn udp_loopback_delivers_packet() {
        // 绑本地端口做接收方，验证 packet 实际发出（不 mock 网络，对齐 testing/strategy.md）。
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = receiver.local_addr().unwrap();
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

        let send = tokio::spawn(async move { send_with_retry(&mac, addr).await });
        let mut buf = [0u8; PACKET_SIZE];
        let (n, _) = receiver.recv_from(&mut buf).await.unwrap();
        send.await.unwrap().unwrap();

        assert_eq!(n, PACKET_SIZE);
        assert_eq!(&buf[..], &magic_packet(&mac));
    }

    #[tokio::test]
    async fn send_wol_full_path_decrypts_and_sends() {
        // 共用高层函数 send_wol：RSA 加密 MAC → send_wol 解密 → 发包 → 接收方收到。
        // 验证手动 wol 命令与巴法触发共用的完整链路。
        use base64::Engine;
        use rsa::sha2::Sha256;
        use rsa::{Oaep, RsaPrivateKey};

        let mut rng = rand::rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = rsa::RsaPublicKey::from(&priv_key);
        let mac = "AA:BB:CC:DD:EE:FF";
        let ct = pub_key
            .encrypt(&mut rng, Oaep::<Sha256>::new(), mac.as_bytes())
            .unwrap();
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(ct);

        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = receiver.local_addr().unwrap();

        let priv_clone = RsaPrivateKey::clone(&priv_key);
        let ct_b64_clone = ct_b64.clone();
        let send = tokio::spawn(async move {
            send_wol(
                &ct_b64_clone,
                &priv_clone,
                addr,
                1,
                Duration::from_millis(0),
            )
            .await
        });
        let mut buf = [0u8; PACKET_SIZE];
        let (n, _) = receiver.recv_from(&mut buf).await.unwrap();
        let result = send.await.unwrap();

        assert!(result.is_ok(), "send_wol should succeed: {result:?}");
        assert_eq!(n, PACKET_SIZE);
        // 包内容正确（mac 解析后构造）
        let expected = magic_packet(&parse_mac(mac).unwrap());
        assert_eq!(&buf[..], &expected);
    }

    #[tokio::test]
    async fn send_wol_invalid_ciphertext_fails() {
        // send_wol 对无效 base64/密文返错误字符串（不 panic），调用方据此填失败结果。
        use rsa::RsaPrivateKey;

        let mut rng = rand::rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let addr: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let result = send_wol(
            "not-valid-ciphertext!!!",
            &priv_key,
            addr,
            1,
            Duration::from_millis(0),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RSA decryption failed"));
    }
}
