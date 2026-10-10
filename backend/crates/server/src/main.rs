//! wakewake-server 组合根（architecture/onion-architecture.md）。
//!
//! 装配 repo → service → handler，启动 axum（h2c，features=["http2"]）。
//! graceful shutdown（drain SSE 连接）。

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::middleware::{from_fn, from_fn_with_state};
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

use wakewake_server::config::{Cli, Command, Settings};
use wakewake_server::db;
use wakewake_server::hub::Hub;
use wakewake_server::integrations::ProviderRegistry;
use wakewake_server::management;
use wakewake_server::middleware::agent_auth::pairing_code_middleware;
use wakewake_server::middleware::auth::jwt_middleware;
use wakewake_server::observability;
use wakewake_server::routes;
use wakewake_server::service::command_handler::WakeWriter;
use wakewake_server::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Command::Version) => {
            println!("wakewake-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        },
        Some(Command::Admin { action }) => management::run_admin(action, &cli).await,
        // 不传子命令 = serve（向后兼容）。
        None | Some(Command::Serve { .. }) => run_server(cli).await,
    }
}

/// 启动 HTTP server（默认/`serve` 子命令）。
async fn run_server(cli: Cli) -> anyhow::Result<()> {
    let settings = Settings::load(&cli)?;

    // 初始化可观测性：tracing-appender 文件轮转 + OTel（OTLP 或本地 file exporter）。
    // guard 必须 hold 到进程退出，Drop 时 flush 所有 buffer + 关闭 provider。
    let _tracing_guard = observability::init_tracing(&settings.log_dir(), &settings.log.level);
    observability::metrics::init_metrics();

    tracing::info!(host = %settings.server.host, port = settings.server.port, "starting wakewake-server");

    // DB pool（max_connections=20 对齐 PG max_connections，perf-est.md §10.3）
    let pool = db::create_pool(&settings.database.dsn).await?;
    db::run_migrations(&pool).await?;

    // 默认 admin 账户（幂等：不存在才创建）。
    management::ensure_bootstrap_admin(&pool, &settings.security).await?;

    // wake writer channel（异步 buffered，database-design.md §wakes）
    let (wake_tx, wake_rx) = tokio::sync::mpsc::channel(256);
    spawn_wake_writer(pool.clone(), wake_rx);

    // Hub（内存命令派发）+ sweep task（60s 过期 + 写 wake(expired) 记录）
    let hub = Hub::new();
    let _sweep = hub.clone().spawn_sweep(pool.clone(), wake_tx.clone());

    // device-sync-v3 §5.5：5s gap 重推 task（投影通道收敛机制）。
    // 每 5s tick：纯内存比较找落后 agent（last_acked < last_pushed）→ 共享一次读 DB 快照重推。
    let gap_pool = pool.clone();
    let _gap_repush = hub.clone().spawn_gap_repush(move |agent_id| {
        let pool = gap_pool.clone();
        Box::pin(async move {
            wakewake_server::service::state::build_for_agent(&pool, agent_id)
                .await
                .ok()
                .map(|snap| {
                    let v = i64::try_from(snap.version).unwrap_or(0);
                    (snap, v)
                })
        })
    });

    // Provider registry（启动时编译 jsonschema，复用）
    let providers = ProviderRegistry::build()?;

    // 运行时服务族（mailer 总闸/预算、PoW 难度旋钮、IP 封禁、登录锁定），
    // 各句柄的持久化文件与恢复语义见各自模块（admin-risk-controls WRFC）。
    let RuntimeServices {
        mailer,
        mailer_control,
        pow,
        login_lockout,
        ip_bans,
        maintenance,
    } = build_runtime_services(&settings);
    // PoW challenge GC task（清理过期/已消费 challenge）
    let pow_gc = pow.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_mins(1));
        loop {
            interval.tick().await;
            pow_gc.gc().await;
        }
    });
    // 登录锁定 GC task（清理过期失败计数，防内存增长）
    let lockout_gc = login_lockout.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_mins(5));
        loop {
            interval.tick().await;
            lockout_gc.gc();
        }
    });

    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port).parse()?;

    // login_events 清理任务（24h 周期，删 30 天前记录，ui-ux-risk-control §8.3）。
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        // 24h login_events 清理周期（§8.3）。
        #[allow(clippy::duration_suboptimal_units)]
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86_400));
        loop {
            interval.tick().await;
            if let Err(e) = sqlx::query(
                "DELETE FROM login_events WHERE created_at < now() - interval '30 days'",
            )
            .execute(&cleanup_pool)
            .await
            {
                tracing::warn!(error = ?e, "login_events cleanup failed");
            }
        }
    });

    // 未验证账号清理 + 封禁条目 housekeeping（admin-risk-controls WRFC）。
    spawn_risk_housekeeping(
        pool.clone(),
        ip_bans.clone(),
        settings.security.unverified_retention_days,
    );

    let state = Arc::new(AppState::new(
        pool,
        settings.clone(),
        hub,
        providers,
        mailer,
        pow,
        login_lockout,
        wake_tx,
        maintenance,
        mailer_control,
        ip_bans,
    ));

    let app = build_router(state.clone(), Arc::new(settings));

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening (h2c)");

    // axum::serve 用 hyper-util auto，自动检测 h2c preface（与 Caddy versions h2c 兼容）。
    // connect_info 供 PeerIp 限流（into_make_service_with_connect_info）。
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    tracing::info!("shutdown complete");
    Ok(())
}

/// 组装 Router：公开（auth）+ JWT 域 + Agent M2M 域（pairing-code）+ health。
fn build_router(state: Arc<AppState>, settings: Arc<Settings>) -> Router {
    // rate_limit.disabled=true（e2e.md §2.5/§4.6 + load.md §9.2）：旁路 governor 速率限制。
    // 仅旁路 per-IP/per-user 频率，不旁路业务配额（MAX_DEVICES_PER_USER 等硬编码 const）。
    let rate_limit_disabled = settings.rate_limit.disabled;
    // 限流 key 口径（A-05B + cloudflare-edge WRFC）：trust_proxy + 权威头（如
    // CF-Connecting-IP）与 §11.C.2 extract_client_ip 同口径。
    let trust_proxy = settings.server.trust_proxy;
    let client_ip_header = settings.server.client_ip_header.clone();

    // 公开认证端点（per-IP 限流：register/login/refresh 10/min、password-reset 3/hour）。
    // 维护中间件挂 public_auth 域：拦截 register（§0.4）。
    let public_auth = routes::auth::routes_with_rate_limit(
        rate_limit_disabled,
        trust_proxy,
        client_ip_header.as_deref(),
    )
    .layer(from_fn_with_state(
        state.clone(),
        wakewake_server::middleware::maintenance::maintenance_public,
    ));

    // JWT 域（浏览器用户端点）。
    // jwt_middleware 现在用 State<Arc<AppState>>（is_active moka 缓存，authentication.md §十）。
    // 维护中间件挂 jwt_routes 域（jwt_middleware 之后，AuthUser 已可用）：拦截 readonly 写方法 / full 非管理员全部（§0.4）。
    let jwt_routes = routes::user::routes()
        .merge(routes::devices::routes())
        .merge(routes::agents::routes())
        .merge(routes::integrations::routes())
        .merge(routes::commands::routes())
        .merge(routes::wakes::routes())
        .layer(from_fn_with_state(
            state.clone(),
            wakewake_server::middleware::maintenance::maintenance_jwt,
        ))
        .layer(from_fn_with_state(state.clone(), jwt_middleware));

    // Admin 域（JWT + superuser 守卫，api-design.md §admin）。
    // 顺序：jwt_middleware（认证 + is_active）→ admin_guard（is_superuser 校验，非 admin 返 404）。
    let admin_routes = routes::admin::routes()
        .layer(from_fn(
            wakewake_server::middleware::admin_guard::admin_guard,
        ))
        .layer(from_fn_with_state(state.clone(), jwt_middleware));

    // Agent M2M 域（pairing-code，/agents/self/*）
    let agent_routes = routes::sse::routes()
        .merge(routes::agent_api::routes())
        .layer(from_fn_with_state(
            state.pool.clone(),
            pairing_code_middleware,
        ));

    let router = Router::new()
        .merge(public_auth)
        .merge(jwt_routes)
        .merge(admin_routes)
        .merge(agent_routes)
    // 请求时长直方图（http_request_duration_seconds）。route_layer 挂载：
    // 路由已匹配，MatchedPath 可用，path 标签是路由模板而非原始 URI（防
    // ID 打爆序列基数）。在限流层之内——governor 的 429 不经过这里，其量
    // 由 http_rate_limited_total 承载。
    .route_layer(axum::middleware::from_fn(
        wakewake_server::middleware::http_metrics::http_metrics,
    ));
    // 全局兜底限流（disabled=true 时跳过）。health 后置 merge 豁免：
    // Docker healthcheck（10s）+ Caddy upstream 探测（10s）合计 12 req/min，
    // 不应占用限流预算，且限流故障时探活必须永远可用。
    let router = wakewake_server::middleware::rate_limit::apply_global_rate_limit(
        router,
        rate_limit_disabled,
        trust_proxy,
        client_ip_header.as_deref(),
    )
    // 应用层 IP 封禁（admin-risk-controls WRFC）：挂全局限流之外（banned IP 不占
    // governor 预算），health 同样后置豁免（探活不带业务语义）。fail-open 见中间件。
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        wakewake_server::middleware::ip_ban::ip_ban_middleware,
    ))
    .merge(routes::health::routes());
    // 所有 API 路由挂载在 /api/v1 前缀下（api-design.md §0.3 版本策略）。
    // routes 用相对路径定义（/devices, /health），统一 nest 到 /api/v1。
    //
    // TraceLayer（cloudflare-edge WRFC）：默认 span/响应事件均 DEBUG 级且不含 IP，
    // 生产 log.level=info 下请求日志不可见——定制为 INFO 级 span，带 client_ip
    // （与限流/审计同口径）+ cf-ray（CF 侧对账键，无 CF 时为 "-"）。
    let span_trust_proxy = trust_proxy;
    let span_ip_header = client_ip_header;
    Router::new()
        .nest("/api/v1", router)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(move |req: &axum::http::Request<axum::body::Body>| {
                    let client_ip = req
                        .extensions()
                        .get::<axum::extract::ConnectInfo<SocketAddr>>()
                        .map(|ci| ci.0)
                        .and_then(|peer| {
                            wakewake_server::util::client_ip_from_headers(
                                req.headers(),
                                peer,
                                span_ip_header.as_deref(),
                                span_trust_proxy,
                            )
                        })
                        .map_or_else(|| "-".to_string(), |ip| ip.to_string());
                    let cf_ray = req
                        .headers()
                        .get("cf-ray")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!(
                        "http_request",
                        client_ip = %client_ip,
                        cf_ray,
                        method = %req.method(),
                        uri = %req.uri(),
                    )
                })
                .on_response(
                    |resp: &axum::http::Response<axum::body::Body>,
                     duration: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            status = %resp.status(),
                            duration_ms = duration.as_millis() as u64,
                            "request completed"
                        );
                    },
                ),
        )
        .with_state(state)
}

/// 组合根可用的运行时服务族（一次性构造，避免 run_server 膨胀）。
struct RuntimeServices {
    mailer: wakewake_server::service::mailer_service::MailerService,
    mailer_control: wakewake_server::service::mailer_control::MailerControl,
    pow: wakewake_server::service::pow::PowService,
    login_lockout: wakewake_server::service::login_lockout::LoginLockout,
    ip_bans: wakewake_server::service::ip_ban::IpBanStore,
    maintenance: wakewake_server::service::maintenance::MaintenanceHandle,
}

fn build_runtime_services(settings: &wakewake_server::config::Settings) -> RuntimeServices {
    // 发信运行态（总闸 + 分路日预算）：data/mailer.json 恢复。
    let mailer_control = wakewake_server::service::mailer_control::MailerControl::load_or_init(
        wakewake_server::service::mailer_control::MailLimits {
            register: settings.mailer.max_register_emails_per_day,
            resend: settings.mailer.max_resend_emails_per_day,
            reset: settings.mailer.max_reset_emails_per_day,
        },
        std::path::PathBuf::from("data/mailer.json"),
    );
    let mailer = wakewake_server::service::mailer_service::MailerService::new(
        &settings.mailer,
        mailer_control.clone(),
    );
    // PoW：difficulty 运行时旋钮持久化 data/pow.json（跨重启保持）。
    let pow = wakewake_server::service::pow::PowService::new(
        &settings.pow,
        Some(std::path::PathBuf::from("data/pow.json")),
    );
    // 维护模式运行态（从 data/maintenance.json 恢复，或用配置默认值）。
    let maintenance = wakewake_server::service::maintenance::MaintenanceHandle::load_or_init(
        settings.maintenance.enabled,
        settings.maintenance.mode,
        &settings.maintenance.message,
        std::path::PathBuf::from("data/maintenance.json"),
    );
    RuntimeServices {
        mailer,
        mailer_control,
        pow,
        maintenance,
        login_lockout: wakewake_server::service::login_lockout::LoginLockout::new(),
        ip_bans: wakewake_server::service::ip_ban::IpBanStore::load_or_init(
            std::path::PathBuf::from("data/ip_bans.json"),
        ),
    }
}

/// 后台 task：风控 housekeeping（24h 周期，首 tick 立即触发 → 启动即清）。
/// - IP 封禁过期条目回收（compact）。
/// - 未验证账号清理：retention_days=0 关闭；守卫与批删见 user_repo::purge_unverified。
fn spawn_risk_housekeeping(
    pool: sqlx::PgPool,
    ip_bans: wakewake_server::service::ip_ban::IpBanStore,
    retention_days: u32,
) {
    tokio::spawn(async move {
        #[allow(clippy::duration_suboptimal_units)]
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86_400));
        loop {
            interval.tick().await;
            ip_bans.compact();
            if retention_days == 0 {
                continue;
            }
            match wakewake_server::repo::user_repo::purge_unverified(
                &pool,
                i32::try_from(retention_days).unwrap_or(i32::MAX),
            )
            .await
            {
                Ok(0) => {},
                Ok(n) => {
                    tracing::info!(count = n, "purged unverified accounts");
                    wakewake_server::observability::metrics::record_unverified_purged(n);
                },
                Err(e) => tracing::warn!(error = ?e, "unverified purge failed"),
            }
        }
    });
}

/// 后台 task：消费 wake writer channel，批量落库（不阻塞命令 ack 链路）。
fn spawn_wake_writer(
    pool: sqlx::PgPool,
    mut rx: tokio::sync::mpsc::Receiver<wakewake_server::domain::wake::RecordWake>,
) {
    tokio::spawn(async move {
        while let Some(wake) = rx.recv().await {
            if let Err(e) = write_wake(&pool, &wake).await {
                tracing::warn!(error = ?e, "wake write failed");
            }
        }
    });
}

async fn write_wake(
    pool: &sqlx::PgPool,
    wake: &wakewake_server::domain::wake::RecordWake,
) -> Result<(), sqlx::Error> {
    use wakewake_server::domain::wake::{WakeStatus, WakeType};
    // RecordWake 的 user_id 由 command_handler 填充（需查 agent.user_id）
    sqlx::query(
        "INSERT INTO wakes (user_id, device_did, device_name, type, status, message)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(wake.user_id)
    .bind(wake.device_did)
    .bind(&wake.device_name)
    .bind(wake.r#type.as_str())
    .bind(wake.status.as_str())
    .bind(wake.message.as_deref())
    .execute(pool)
    .await?;
    wakewake_server::observability::metrics::record_wake(wake.status.as_str());
    // 抑制未用 import
    let _ = (WakeStatus::Success, WakeType::Wol);
    Ok(())
}

/// 优雅关停（SIGTERM/SIGINT，drain SSE 连接）。
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received");
}

// WakeWriter 类型别名已在 service::command_handler 定义，main 直接用。
#[allow(dead_code)]
fn _wake_writer_type_hint(_: WakeWriter) {}
