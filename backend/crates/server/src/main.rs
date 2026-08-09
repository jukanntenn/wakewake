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
    let _tracing_guard = observability::init_tracing(&settings.log_dir());
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

    // Mailer（密码重置邮件，enabled=false 时端点返 503）+ PoW（防滥用）+ 登录失败锁定
    let mailer = wakewake_server::service::mailer_service::MailerService::new(&settings.mailer);
    let pow = wakewake_server::service::pow::PowService::new(&settings.pow);
    let login_lockout = wakewake_server::service::login_lockout::LoginLockout::new();
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
    let state = Arc::new(AppState::new(
        pool,
        settings.clone(),
        hub,
        providers,
        mailer,
        pow,
        login_lockout,
        wake_tx,
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

    // 公开认证端点（per-IP 限流：register/login/refresh 10/min、password-reset 3/hour）。
    let public_auth = routes::auth::routes_with_rate_limit(rate_limit_disabled);

    // JWT 域（浏览器用户端点）。
    // jwt_middleware 现在用 State<Arc<AppState>>（is_active moka 缓存，authentication.md §十）。
    let jwt_routes = routes::user::routes()
        .merge(routes::devices::routes())
        .merge(routes::agents::routes())
        .merge(routes::integrations::routes())
        .merge(routes::commands::routes())
        .merge(routes::wakes::routes())
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
        .merge(routes::health::routes());
    // 全局兜底限流（disabled=true 时跳过）
    let router = wakewake_server::middleware::rate_limit::apply_global_rate_limit(
        router,
        rate_limit_disabled,
    );
    // 所有 API 路由挂载在 /api/v1 前缀下（api-design.md §0.3 版本策略）。
    // routes 用相对路径定义（/devices, /health），统一 nest 到 /api/v1。
    Router::new()
        .nest("/api/v1", router)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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
