//! axum-governor 限流配置（new-design.md §7.5 / authentication.md §七层1）。
//!
//! 多层配置：auth 端点 per-IP；password-reset per-IP 3/hour；全局兜底 per-Global。
//! 返回类型显式标注 Key 类型（GovernorLayer<K>）。
//!
//! `rate_limit.disabled=true`（e2e.md §2.5/§4.6 + load.md §9.2）跳过挂载所有 governor
//! `layer——仅旁路速率限制，不旁路业务配额（MAX_DEVICES_PER_USER` 等硬编码 const）。

use std::net::IpAddr;
use std::time::Duration;

use axum::Router;
use axum_governor::{
    GovernorConfigBuilder, GovernorLayer, KeyExtractor, KeyOutcome, Quota,
    extractor::{Global, PeerIp},
};

/// auth 端点限流：per-IP，10 req/min（PeerIp 需 `connect_info`）。
#[must_use]
pub fn auth_rate_limit_layer() -> GovernorLayer<IpAddr> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(PeerIp::default())
        .expect_connect_info()
        .quota_default(Quota::requests_per_minute(axum_governor::nz!(10u32)))
        .max_keys(50_000)
        .gc_interval(Duration::from_mins(1))
        .finish()
        .expect("auth rate limit config");
    GovernorLayer::new(cfg)
}

/// password-reset 端点限流：per-IP，3 req/hour。
#[must_use]
pub fn password_reset_rate_limit_layer() -> GovernorLayer<IpAddr> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(PeerIp::default())
        .expect_connect_info()
        .quota_default(Quota::requests_per_hour(axum_governor::nz!(3u32)))
        .max_keys(50_000)
        .gc_interval(Duration::from_mins(1))
        .finish()
        .expect("password reset rate limit config");
    GovernorLayer::new(cfg)
}

/// 全局兜底限流（Global，120 req/min）。
#[must_use]
pub fn global_rate_limit_layer() -> GovernorLayer<()> {
    let cfg = GovernorConfigBuilder::default()
        .with_extractor(Global)
        .quota_default(Quota::requests_per_minute(axum_governor::nz!(120u32)))
        .max_keys(50_000)
        .finish()
        .expect("global rate limit config");
    GovernorLayer::new(cfg)
}

/// 条件挂载 auth 端点限流：`disabled=true` 时跳过（e2e.md §2.5）。
/// 泛型 S：governor layer 不依赖 router state，保留原 state 类型。
pub fn apply_auth_rate_limit<S>(router: Router<S>, disabled: bool) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(auth_rate_limit_layer())
    }
}

/// 条件挂载 password-reset 端点限流。
pub fn apply_password_reset_rate_limit<S>(router: Router<S>, disabled: bool) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(password_reset_rate_limit_layer())
    }
}

/// 条件挂载全局兜底限流。
pub fn apply_global_rate_limit<S>(router: Router<S>, disabled: bool) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if disabled {
        router
    } else {
        router.layer(global_rate_limit_layer())
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
