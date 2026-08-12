//! 跨层纯工具函数（无项目内依赖，可被任意层调用）。

/// 对字符串做最小化的 query 值百分号编码。
///
/// 仅编码会对 URL 解析造成歧义的字符（`&`/`=`/`#`/`+`/` `/`%`），
/// 其余保持原样。verify/reset token 是 base64url 字符集（`A-Za-z0-9-_`），
/// 实际不会被编码——此函数为防御性，覆盖将来 query 值含特殊字符的场景。
fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'&' | b'=' | b'#' | b'+' | b' ' | b'%' | b'"' | b'<' | b'>' => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            },
            // 控制字符（0x00-0x1F）与 0x7F 以下 DEL 也编码。
            0x00..=0x1F | 0x7F => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            },
            _ => out.push(byte as char),
        }
    }
    out
}

/// 拼接绝对 URL：`base` + `path` + `query`。
///
/// - `base`：含 scheme 的绝对地址（如 `https://wakewake.app`），尾斜杠会被去除。
/// - `path`：以 `/` 开头的路径（如 `/verify-email`）。
/// - `query`：`(key, value)` 有序切片，按顺序拼成 `?k1=v1&k2=v2`；空切片则无 query。
///
/// `base` 不以 `http://`/`https://` 开头时返回 `None`（视为配置非法）。
#[must_use]
pub fn build_absolute_url(base: &str, path: &str, query: &[(&str, &str)]) -> Option<String> {
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return None;
    }
    let trimmed_base = base.trim_end_matches('/');
    let mut url = format!("{trimmed_base}{path}");
    if !query.is_empty() {
        let qs = query
            .iter()
            .map(|(k, v)| format!("{}={}", encode_query_value(k), encode_query_value(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&qs);
    }
    Some(url)
}

/// 从请求头提取真实客户端 IP（ui-ux-risk-control §11.C.2，反代场景）。
///
/// 优先级：X-Forwarded-For 最左 → X-Real-IP → TCP 对端。
/// `trust_proxy=false` 时跳过 XFF/X-Real-IP，直接用 TCP 对端（防伪造）。
/// XFF 格式："203.0.113.8, 10.0.0.1" → 取最左（最原始客户端）。
#[must_use]
pub fn extract_client_ip(
    xff: Option<&str>,
    x_real_ip: Option<&str>,
    peer: std::net::SocketAddr,
    trust_proxy: bool,
) -> Option<std::net::IpAddr> {
    if trust_proxy {
        if let Some(xff) = xff {
            if let Some(first) = xff.split(',').next() {
                if let Ok(ip) = first.trim().parse::<std::net::IpAddr>() {
                    return Some(ip);
                }
            }
        }
        if let Some(real) = x_real_ip {
            if let Ok(ip) = real.trim().parse::<std::net::IpAddr>() {
                return Some(ip);
            }
        }
    }
    Some(peer.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_with_query() {
        let url = build_absolute_url(
            "https://wakewake.app",
            "/verify-email",
            &[("token", "abc.123")],
        );
        assert_eq!(
            url.as_deref(),
            Some("https://wakewake.app/verify-email?token=abc.123")
        );
    }

    #[test]
    fn strips_trailing_slash() {
        let url = build_absolute_url(
            "https://wakewake.app/",
            "/reset-password",
            &[("token", "x")],
        );
        assert_eq!(
            url.as_deref(),
            Some("https://wakewake.app/reset-password?token=x")
        );
    }

    #[test]
    fn with_port() {
        let url = build_absolute_url(
            "http://192.168.5.200:8449",
            "/verify-email",
            &[("token", "t")],
        );
        assert_eq!(
            url.as_deref(),
            Some("http://192.168.5.200:8449/verify-email?token=t")
        );
    }

    #[test]
    fn empty_query_no_question_mark() {
        let url = build_absolute_url("https://app.example", "/path", &[]);
        assert_eq!(url.as_deref(), Some("https://app.example/path"));
    }

    #[test]
    fn multiple_query_params() {
        let url = build_absolute_url("https://app.example", "/p", &[("a", "1"), ("b", "2")]);
        assert_eq!(url.as_deref(), Some("https://app.example/p?a=1&b=2"));
    }

    #[test]
    fn encodes_special_chars_in_value() {
        let url = build_absolute_url("https://app.example", "/p", &[("q", "a&b=c#d e")]);
        assert_eq!(
            url.as_deref(),
            Some("https://app.example/p?q=a%26b%3Dc%23d%20e")
        );
    }

    #[test]
    fn base64url_token_not_encoded() {
        // verify/reset token 是 base64url，不应被改写
        let token = "MQ.tj73fj.01000e58646206b148ce";
        let url = build_absolute_url("https://app.example", "/verify-email", &[("token", token)]);
        assert_eq!(
            url.as_deref(),
            Some("https://app.example/verify-email?token=MQ.tj73fj.01000e58646206b148ce")
        );
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert_eq!(build_absolute_url("ftp://example.com", "/p", &[]), None);
        assert_eq!(build_absolute_url("example.com", "/p", &[]), None);
        assert_eq!(build_absolute_url("/relative", "/p", &[]), None);
        assert_eq!(build_absolute_url("", "/p", &[]), None);
    }

    #[test]
    fn extract_client_ip_xff_first() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let peer: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        let ip = extract_client_ip(Some("203.0.113.8, 10.0.0.1"), None, peer, true);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 8))));
    }

    #[test]
    fn extract_client_ip_x_real_ip_fallback() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let peer: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        let ip = extract_client_ip(None, Some("198.51.100.5"), peer, true);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 5))));
    }

    #[test]
    fn extract_client_ip_no_trust_falls_back_to_peer() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let peer: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        // trust_proxy=false → 忽略 XFF，用 peer
        let ip = extract_client_ip(Some("203.0.113.8"), None, peer, false);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn extract_client_ip_no_headers_uses_peer() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let peer: SocketAddr = "192.168.1.1:80".parse().unwrap();
        let ip = extract_client_ip(None, None, peer, true);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
    }

    #[test]
    fn extract_client_ip_invalid_xff_falls_back() {
        use std::net::SocketAddr;
        let peer: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        // XFF 非 IP 格式 → 跳过，回退 peer
        let ip = extract_client_ip(Some("not-an-ip"), None, peer, true);
        assert!(ip.is_some());
    }
}
