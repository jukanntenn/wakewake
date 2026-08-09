//! AppState（组合根装配的共享状态）。
//!
//! DB pool、Hub（命令派发）、各 service 依赖、provider registry、moka state 缓存、wake writer。

use std::sync::Arc;

use moka::future::Cache;
use sqlx::PgPool;
use xxhash_rust::xxh3::Xxh3DefaultBuilder;

use crate::config::Settings;
use crate::hub::Hub;
use crate::integrations::ProviderRegistry;
use crate::service::command_handler::WakeWriter;
use crate::service::login_lockout::LoginLockout;
use crate::service::mailer_service::MailerService;
use crate::service::pow::PowService;
use wakewake_protocol::StateSnapshot;

/// 共享应用状态（Clone 廉价，内部 Arc）。
#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<PgPool>,
    pub settings: Arc<Settings>,
    pub hub: Hub,
    pub providers: Arc<ProviderRegistry>,
    pub mailer: MailerService,
    pub pow: PowService,
    pub login_lockout: LoginLockout,
    /// per-agent state 快照缓存（设备/集成变更时失效，hasher 用 xxhash）。
    pub state_cache: Cache<i64, StateSnapshot, Xxh3DefaultBuilder>,
    /// `user_id` → `is_active` 缓存（JWT 中间件查 `is_active` 用，TTL 5s，authentication.md §十）。
    /// 5s TTL 是 DB 压力与禁用生效延迟的折中：管理员禁用后最长 5s 内旧 access token 仍可用。
    pub user_active_cache: Cache<i64, bool, Xxh3DefaultBuilder>,
    pub wake_writer: WakeWriter,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        pool: PgPool,
        settings: Settings,
        hub: Hub,
        providers: ProviderRegistry,
        mailer: MailerService,
        pow: PowService,
        login_lockout: LoginLockout,
        wake_writer: WakeWriter,
    ) -> Self {
        Self {
            pool: Arc::new(pool),
            settings: Arc::new(settings),
            hub,
            providers: Arc::new(providers),
            mailer,
            pow,
            login_lockout,
            // moka 用 xxhash 替代默认 SipHash（§4.6），per-agent 缓存，TTL 5min。
            state_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_mins(5))
                .max_capacity(100_000)
                .build_with_hasher(Xxh3DefaultBuilder::new()),
            // JWT 中间件 is_active 缓存（authentication.md §十），TTL 5s。
            user_active_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(5))
                .max_capacity(100_000)
                .build_with_hasher(Xxh3DefaultBuilder::new()),
            wake_writer,
        }
    }
}
