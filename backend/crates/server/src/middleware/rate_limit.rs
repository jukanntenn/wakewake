//! axum-governor 限流配置（new-design.md §7.5 / authentication.md §七层1）。
//!
//! 多层配置：auth 端点 per-IP；password-reset per-IP 3/hour；全局兜底 per-IP 600/min。
//! 返回类型显式标注 Key 类型（GovernorLayer<K>）。
//!
//! `rate_limit.disabled=true`（e2e.md §2.5/§4.6 + load.md §9.2）跳过挂载所有 governor
//! `layer——仅旁路速率限制，不旁路业务配额（MAX_DEVICES_PER_USER` 等硬编码 const）。
//!
//! 限流 key 口径（A-05B + cloudflare-edge WRFC）：`ClientIpExtractor` 与 §11.C.2
//! `extract_client_ip` 同口径——配置了权威头（`client_ip_header`，如 CF-Connecting-IP）
//! 时优先读权威头，否则 XFF 首跳 / X-Real-IP；`trust_proxy=false` 时退化为 TCP 对端 IP。
//! 429 响应统一走 `rate_limit_error_handler` 返 JSON `{code,message}` + Retry-After（UX-70 修复）。

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use axum::Router;
use axum_governor::{
    GovernorConfigBuilder, GovernorLayer, KeyExtractor, KeyOutcome, Quota, error::RejectionReason,
};

use axum::extract::ConnectInfo;
use axum::http;
use axum::http::request::Parts;

use crate::util::client_ip_from_headers;

/// 限流 key 提取器：与 `util::client_ip_from_headers` 同口径（A-05B + cloudflare-edge）。
/// `trust_proxy=true` → 权威头（若配置）→ XFF 首跳 / X-Real-IP；否则 TCP 对端 IP。
#[derive(Clone, Debug)]
pub struct ClientIpExtractor {
    trust_proxy: bool,
    client_ip_header: Option<String>,
}

impl ClientIpExtractor {
    #[must_use]
    pub fn new(trust_proxy: bool, client_ip_header: Option<&str>) -> Self {
        Self {
            trust_proxy,
            client_ip_header: client_ip_header.map(String::from),
        }
    }
}

impl KeyExtractor for ClientIpExtractor {
    type Key = IpAddr;

    fn requires_connect_info(&self) -> bool {
        true
    }

    fn extract(&self, parts: &Parts) -> Result<KeyOutcome<IpAddr>, axum_governor::ExtractionError> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .ok_or(axum_governor::ExtractionError::MissingConnectInfo)?
            .0;
        let ip = client_ip_from_headers(
            &parts.headers,
            peer,
            self.client_ip_header.as_deref(),
            self.trust_proxy,
        )
        .unwrap_or_else(|| peer.ip());
        Ok(KeyOutcome {
            key: ip,
            quota_override: None,
        })
    }
}

/// 429 响应：统一 JSON 错误包络 `{code,message}` + Retry-After（UX-70）。
/// key 提取失败（理论上不应发生，connect_info 已强制）回 500。
/// 拒绝计入进程内 + OTel 计数（risk 面板 / admin-risk-controls WRFC）。
fn rate_limit_error_handler(reason: RejectionReason) -> http::Response<axum::body::Body> {
    crate::observability::metrics::record_rate_limited();
    let (status, wait) = match &reason {
        RejectionReason::QuotaExceeded { wait, .. } => (http::StatusCode::TOO_MANY_REQUESTS, *wait),
        RejectionReason::KeyExtractionFailed(_) => {
            (http::StatusCode::INTERNAL_SERVER_ERROR, Duration::ZERO)
        },
    };
    let body = serde_json::json!({
        "code": "RATE_LIMITED",
        "message": "Too many requests, please try again later.",
    });
    let mut resp = http::Response::new(axum::body::Body::from(body.to_string()));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        "application/json".parse().expect("content-type header"),
    );
    if wait > Duration::ZERO {
        resp.headers_mut().insert(
            http::header::RETRY_AFTER,
            format!("{}", wait.as_secs())
                .parse()
                .expect("retry-after header"),
        );
    }
    resp
}

/// auth 端点限流：per-client-IP，10 req/min（A-05B：trust_proxy + 权威头口径）。
#[must_use]
pub fn auth_rate_limit_layer(
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> GovernorLayer<IpAddr> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(ClientIpExtractor::new(trust_proxy, client_ip_header))
        .expect_connect_info()
        .quota_default(Quota::requests_per_minute(axum_governor::nz!(10u32)))
        .max_keys(50_000)
        .gc_interval(Duration::from_mins(1))
        .error_handler(rate_limit_error_handler)
        .finish()
        .expect("auth rate limit config");
    GovernorLayer::new(cfg)
}

/// password-reset 端点限流：per-client-IP，3 req/hour。
#[must_use]
pub fn password_reset_rate_limit_layer(
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> GovernorLayer<IpAddr> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(ClientIpExtractor::new(trust_proxy, client_ip_header))
        .expect_connect_info()
        .quota_default(Quota::requests_per_hour(axum_governor::nz!(3u32)))
        .max_keys(50_000)
        .gc_interval(Duration::from_mins(1))
        .error_handler(rate_limit_error_handler)
        .finish()
        .expect("password reset rate limit config");
    GovernorLayer::new(cfg)
}

/// 全局兜底限流：per-client-IP，600 req/min。
///
/// 曾为 Global 键 120 req/min（全服务器单桶）：仪表盘轮询 ≈40 req/min/页，
/// 3 个并发页面即触发 429，agent sync / health 探测也同桶竞争。改 per-IP 后
/// 单页 15 倍余量、正常用户不可达，单 IP 洪泛的 DoS 兜底语义保留。
#[must_use]
pub fn global_rate_limit_layer(
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> GovernorLayer<IpAddr> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(ClientIpExtractor::new(trust_proxy, client_ip_header))
        .expect_connect_info()
        .quota_default(Quota::requests_per_minute(axum_governor::nz!(600u32)))
        .max_keys(50_000)
        .gc_interval(Duration::from_mins(1))
        .error_handler(rate_limit_error_handler)
        .finish()
        .expect("global rate limit config");
    GovernorLayer::new(cfg)
}

/// 条件挂载 auth 端点限流：`disabled=true` 时跳过（e2e.md §2.5）。
/// 泛型 S：governor layer 不依赖 router state，保留原 state 类型。
pub fn apply_auth_rate_limit<S>(
    router: Router<S>,
    disabled: bool,
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(auth_rate_limit_layer(trust_proxy, client_ip_header))
    }
}

/// 条件挂载 password-reset 端点限流。
pub fn apply_password_reset_rate_limit<S>(
    router: Router<S>,
    disabled: bool,
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(password_reset_rate_limit_layer(
            trust_proxy,
            client_ip_header,
        ))
    }
}

/// 条件挂载全局兜底限流。
pub fn apply_global_rate_limit<S>(
    router: Router<S>,
    disabled: bool,
    trust_proxy: bool,
    client_ip_header: Option<&str>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(global_rate_limit_layer(trust_proxy, client_ip_header))
    }
}

/// 从 request extensions 取 `AuthUser` 的 `user_id` 作限流 key。
/// auth 中间件已注入 `crate::middleware::auth::AuthUser`。
///
/// 注：当前未挂载（per-user 限流如 refresh 60/min/user、wake 30/min/user 是
/// api-design.md §1.6 规划项，留作未来扩展）。保留实现避免重复造轮子。
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct UserExt;

impl KeyExtractor for UserExt {
    type Key = i64;

    fn extract(
        &self,
        parts: &axum::http::request::Parts,
    ) -> Result<KeyOutcome<Self::Key>, axum_governor::ExtractionError> {
        let user = parts.extensions.get::<crate::middleware::auth::AuthUser>();
        match user {
            Some(u) => Ok(KeyOutcome {
                key: u.user_id,
                quota_override: None,
            }),
            None => Err(axum_governor::ExtractionError::Other(
                "no auth user in extensions".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_uses_xff_first_hop_when_trusted() {
        let mut req = http::Request::new(());
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>("10.0.0.1:0".parse().unwrap()));
        req.headers_mut()
            .insert("x-forwarded-for", "203.0.113.7, 10.0.0.2".parse().unwrap());
        let parts = req.into_parts().0;
        let key = ClientIpExtractor::new(true, None)
            .extract(&parts)
            .unwrap()
            .key;
        assert_eq!(key, "203.0.113.7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn client_ip_prefers_configured_header_over_spoofed_xff() {
        // CF 拓扑（cloudflare-edge WRFC）：XFF 是追加链（`伪造, 访客, CF边缘`），
        // 最左可伪造；权威头 CF-Connecting-IP 恒为单个真实访客 IP。
        let mut req = http::Request::new(());
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>("104.16.1.1:0".parse().unwrap()));
        req.headers_mut().insert(
            "x-forwarded-for",
            "6.6.6.6, 203.0.113.8, 104.16.1.1".parse().unwrap(),
        );
        req.headers_mut()
            .insert("cf-connecting-ip", "203.0.113.8".parse().unwrap());
        let parts = req.into_parts().0;
        let key = ClientIpExtractor::new(true, Some("CF-Connecting-IP"))
            .extract(&parts)
            .unwrap()
            .key;
        assert_eq!(key, "203.0.113.8".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn client_ip_uses_peer_when_not_trusted() {
        let mut req = http::Request::new(());
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>("203.0.113.9:0".parse().unwrap()));
        req.headers_mut()
            .insert("x-forwarded-for", "198.51.100.1".parse().unwrap());
        let parts = req.into_parts().0;
        let key = ClientIpExtractor::new(false, Some("CF-Connecting-IP"))
            .extract(&parts)
            .unwrap()
            .key;
        // trust_proxy=false → 忽略一切头，用对端 IP
        assert_eq!(key, "203.0.113.9".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn client_ip_falls_back_to_peer_without_headers() {
        let mut req = http::Request::new(());
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>("127.0.0.1:0".parse().unwrap()));
        let parts = req.into_parts().0;
        let key = ClientIpExtractor::new(true, None)
            .extract(&parts)
            .unwrap()
            .key;
        assert_eq!(key, "127.0.0.1".parse::<IpAddr>().unwrap());
    }
}
