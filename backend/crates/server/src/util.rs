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
}
