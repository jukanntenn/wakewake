//! SSE 客户端：连接 + 重连退避（api-design.md §2.10 / §5.8）。
//!
//! 指数退避 base=5s factor=2 max=300s jitter=±50%（防同步重连脉冲）。
//! 401 → 立即终止（pairing code 失效）。
//! 所有非主动断开都走退避（修复旧 agent 优雅 EOF 重置退避的 bug）。

use std::time::Duration;

/// 退避状态机。
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    factor: u32,
    max: Duration,
    current: Duration,
}

impl Backoff {
    /// 5s → 300s + ±50% jitter（api-design.md §2.10）。
    #[must_use]
    pub fn new() -> Self {
        Self::with_params(Duration::from_secs(5), 2, Duration::from_mins(5))
    }

    #[must_use]
    pub fn with_params(base: Duration, factor: u32, max: Duration) -> Self {
        Self {
            base,
            factor,
            max,
            current: base,
        }
    }

    /// 计算下次退避时间（含 ±50% jitter），并翻倍 current（封顶 max）。
    pub fn next_delay(&mut self) -> Duration {
        let jitter = self.random_jitter();
        let delay = self.current + jitter;
        // 翻倍（封顶 max）
        self.current = (self.current * self.factor).min(self.max);
        delay.min(self.max * 2)
    }

    /// 重连成功 → 重置退避。
    pub fn reset(&mut self) {
        self.current = self.base;
    }

    /// ±50% jitter（基于 current）。
    fn random_jitter(&self) -> Duration {
        use getrandom::fill;
        let mut buf = [0u8; 4];
        let _ = fill(&mut buf);
        let n = f64::from(u32::from_le_bytes(buf)) / f64::from(u32::MAX);
        let offset = (n - 0.5) * 2.0;
        let jitter_ms = (offset.abs() * self.current.as_millis() as f64 * 0.5) as u64;
        Duration::from_millis(jitter_ms)
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

/// SSE 连接断开原因。401 → 终止；其他 → 退避。
#[derive(Debug)]
pub enum DisconnectReason {
    /// pairing code 失效（401）→ agent 立即退出，不重连。
    Unauthorized,
    /// 网络断开 / 5xx / EOF → 退避重连。
    Network,
}

/// 单次连接尝试的结局（供 main 循环决策）。
#[derive(Debug)]
pub enum Outcome {
    /// pairing code 失效 → 终止退出。
    Unauthorized,
    /// 网络断开 / 主动断开 → 退避重连。
    Disconnected,
    /// 连接成功（稳态后断开也归为 Disconnected）。
    Connected,
}

/// 简单 SSE 事件解析（从 reqwest stream 逐行读）。
#[derive(Debug, Clone)]
pub enum SseEvent {
    /// event: <name> + data: <json>
    Event {
        event: String,
        id: String,
        data: String,
    },
    /// : <comment>（connected / ping）
    Comment(String),
}

/// 从 SSE 文本块解析事件（SSE 协议：空行分隔事件，event:/id:/data: 行）。
#[must_use]
pub fn parse_sse_event(chunk: &str) -> Option<SseEvent> {
    let mut event_type = String::new();
    let mut id = String::new();
    let mut data = String::new();
    let mut is_comment = false;
    let mut comment = String::new();

    for line in chunk.lines() {
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix(": ") {
            is_comment = true;
            comment = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("event: ") {
            event_type = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("id: ") {
            id = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("data: ") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        }
    }

    if is_comment {
        return Some(SseEvent::Comment(comment));
    }
    if !event_type.is_empty() {
        return Some(SseEvent::Event {
            event: event_type,
            id,
            data,
        });
    }
    None
}

/// 连接 SSE 端点并运行命令处理循环。
/// 返回 Outcome（Unauthorized → 退出，Disconnected → 退避重连）。
pub async fn connect_and_run<F>(
    server_url: &str,
    pairing_code: &str,
    public_key_pem: Option<&str>,
    mut handle_event: F,
) -> Outcome
where
    F: FnMut(SseEvent),
{
    let url = format!("{server_url}/api/v1/agents/self/events");
    let client = crate::http_client::build_http_client();

    let mut req = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .bearer_auth(pairing_code);

    if let Some(pk) = public_key_pem {
        req = req.header("X-Public-Key", pk);
    }

    let response = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, error_debug = ?e, "SSE connect failed");
            return Outcome::Disconnected;
        },
    };

    // 401 → pairing code 失效，立即终止
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        tracing::error!("SSE 401 Unauthorized — pairing code invalid, terminating");
        return Outcome::Unauthorized;
    }
    if !response.status().is_success() {
        tracing::warn!(
            status = response.status().as_u16(),
            "SSE non-200, will retry"
        );
        return Outcome::Disconnected;
    }

    // 流式读取 SSE（reqwest bytes_stream + SSE 协议解析）。
    // §8.6.1 防半开 TCP：agent 若 60s 内未收到任何数据（含 : ping 注释行），判连接死，强制断开重连。
    // server 心跳 30s，故 60s 超时容忍一次心跳丢失。
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(1);

    loop {
        match tokio::time::timeout(READ_TIMEOUT, stream.next()).await {
            Ok(Some(chunk_result)) => match chunk_result {
                Ok(chunk) => {
                    buffer.push_str(&String::from_utf8_lossy(&chunk[..]));
                    // SSE 事件以空行分隔（\n\n）
                    while let Some(idx) = buffer.find("\n\n") {
                        let raw_event = buffer[..idx].to_string();
                        buffer = buffer[idx + 2..].to_string();
                        if let Some(event) = parse_sse_event(&raw_event) {
                            handle_event(event);
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "SSE stream error, will retry");
                    return Outcome::Disconnected;
                },
            },
            Ok(None) => {
                // 优雅 EOF（stream 结束）→ Disconnected（走退避，修复旧 agent EOF 重置退避 bug）
                tracing::warn!("SSE stream ended (EOF), will retry with backoff");
                return Outcome::Disconnected;
            },
            Err(_) => {
                // §8.6.1：60s 读超时 → 判半开 TCP，强制断开重连
                tracing::warn!(
                    "SSE read timeout (60s no data), forcing reconnect (half-open TCP guard)"
                );
                return Outcome::Disconnected;
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        let mut b = Backoff::with_params(Duration::from_secs(5), 2, Duration::from_mins(5));
        let d1 = b.next_delay();
        assert!(d1 >= Duration::from_secs(2) && d1 <= Duration::from_secs(8));
        assert!(b.current >= Duration::from_secs(9));
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut b = Backoff::with_params(Duration::from_secs(5), 2, Duration::from_mins(5));
        for _ in 0..20 {
            b.next_delay();
        }
        assert!(b.current <= Duration::from_mins(5));
    }

    #[test]
    fn backoff_resets() {
        let mut b = Backoff::with_params(Duration::from_secs(5), 2, Duration::from_mins(5));
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.current, Duration::from_secs(5));
    }

    #[test]
    fn parse_state_event() {
        let chunk = "event: state\nid: s_abc123\ndata: {\"devices\":[]}\n";
        let event = parse_sse_event(chunk).unwrap();
        match event {
            SseEvent::Event { event, id, data } => {
                assert_eq!(event, "state");
                assert_eq!(id, "s_abc123");
                assert!(data.contains("devices"));
            },
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn parse_comment_event() {
        let chunk = ": connected\n";
        let event = parse_sse_event(chunk).unwrap();
        match event {
            SseEvent::Comment(c) => assert_eq!(c, "connected"),
            _ => panic!("expected Comment"),
        }
    }

    #[test]
    fn parse_command_event() {
        let chunk = "event: command\nid: c_xyz\ndata: {\"type\":\"wol\"}\n";
        let event = parse_sse_event(chunk).unwrap();
        match event {
            SseEvent::Event { event, id, .. } => {
                assert_eq!(event, "command");
                assert_eq!(id, "c_xyz");
            },
            _ => panic!("expected Event"),
        }
    }
}
