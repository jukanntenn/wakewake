//! wakewake-agent 入口（device-sync-v3 §8.6 / §10）。
//!
//! - 首启生成 RSA 密钥对（持久化 `key.pem`），首连通过 X-Public-Key 上报**纯 base64 体**（§8.6）。
//! - 重连退避 5s→300s + ±50% jitter。
//! - SSE 事件：state（teardown-apply → 上报 applied_version ack + notify 对账调度器）+ command（execute-complete）。
//! - Bemfa：Reconciler 调度器（Notify 驱动，§10.3）+ MQTT loop（watch 增量订阅，§10.5）。

use std::sync::Arc;

use clap::Parser;
use wakewake_agent::bemfa_state::{BemfaCoordinator, ReconcilerDeps};
use wakewake_agent::command_executor;
use wakewake_agent::config::{Cli, Settings};
use wakewake_agent::http_client;
use wakewake_agent::keystore;
use wakewake_agent::reporter;
use wakewake_agent::sse_client::{self, Backoff, Outcome, SseEvent};
use wakewake_agent::state::{AgentState, ApplyOutcome, SharedState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let settings = Settings::load(&cli)?;
    let _log_guard = init_tracing(&settings);

    tracing::info!(server = %settings.server_url, "starting wakewake-agent");

    // 持久化 RSA 密钥对（首启生成）。
    let (private_key, public_key_pem) = keystore::load_or_generate(&settings.home_dir)?;

    // Agent 内存状态
    let state = AgentState::new_shared();

    // HTTP client（sse_client + reporter + 巴法 topic API 共用）
    let http_client = http_client::build_http_client();

    // Bemfa 协调器（agent 单例）。device-sync-v3 §10：Notify 调度器，纯内存。
    let bemfa_coord = Arc::new(BemfaCoordinator::new());

    // 启动对账调度器主循环（§10.3）。spawn 为独立 task，阻塞到进程退出。
    {
        let coord = bemfa_coord.clone();
        let deps = ReconcilerDeps {
            state: state.clone(),
            private_key: private_key.clone(),
            wol_settings: settings.wol.clone(),
            http_client: http_client.clone(),
            server_url: settings.server_url.clone(),
            pairing_code: settings.pairing_code.clone(),
        };
        tokio::spawn(async move {
            coord.run_scheduler(deps).await;
        });
    }

    // SSE client 主循环（重连退避）
    // X-Public-Key（§8.6 PEM 契约）：agent 发送纯 base64 体（剥 PEM 标记 + 所有换行）。
    let public_key_header = strip_pem_to_base64_body(&public_key_pem);
    let mut backoff = Backoff::new();
    loop {
        let outcome = sse_client::connect_and_run(
            &settings.server_url,
            &settings.pairing_code,
            Some(&public_key_header),
            {
                let state = state.clone();
                let settings = settings.clone();
                let http_client = http_client.clone();
                let private_key = private_key.clone();
                let bemfa_coord = bemfa_coord.clone();
                move |event| {
                    handle_sse_event(
                        event,
                        &state,
                        &settings,
                        &http_client,
                        &private_key,
                        &bemfa_coord,
                    );
                }
            },
        )
        .await;

        match outcome {
            Outcome::Unauthorized => {
                tracing::error!("pairing code invalid (401), terminating");
                return Ok(());
            },
            Outcome::Disconnected => {
                let delay = backoff.next_delay();
                tracing::warn!(?delay, "disconnected, backing off");
                tokio::time::sleep(delay).await;
            },
            Outcome::Connected => {
                backoff.reset();
            },
        }
    }
}

/// 处理单个 SSE 事件（同步——在 `connect_and_run` 的回调里执行）。
/// state：teardown-apply → 更新 applied_version → **每次都上报 ack**（即便幂等丢弃，§10.2）
///        → notify 对账调度器。command：execute → complete。
fn handle_sse_event(
    event: SseEvent,
    state: &SharedState,
    settings: &Settings,
    http_client: &reqwest::Client,
    private_key: &rsa::RsaPrivateKey,
    bemfa_coord: &Arc<BemfaCoordinator>,
) {
    let server_url = settings.server_url.clone();
    let pairing_code = settings.pairing_code.clone();
    let client = http_client.clone();

    match event {
        SseEvent::Comment(c) => {
            tracing::debug!(comment = %c, "SSE comment");
        },
        SseEvent::Event { event, id, data } => match event.as_str() {
            "state" => match serde_json::from_str::<wakewake_protocol::StateSnapshot>(&data) {
                Ok(snapshot) => {
                    let version = snapshot.version;
                    let outcome = {
                        let mut s = state.write().expect("state lock");
                        s.apply_snapshot(snapshot)
                    };
                    // §10.2：每次都上报 applied_version（即便幂等丢弃）
                    let applied_version = match outcome {
                        ApplyOutcome::Applied { version } => version,
                        ApplyOutcome::IdempotentDropped { version } => version,
                    };
                    let coord = bemfa_coord.clone();
                    let srv = server_url.clone();
                    let code = pairing_code.clone();
                    let hc = client.clone();
                    let st = state.clone();
                    tokio::spawn(async move {
                        // 上报 ack（applied_version 水位）
                        let req = wakewake_protocol::SyncRequest {
                            applied_version: Some(applied_version),
                            integration: None,
                        };
                        let _ = reporter::sync(&hc, &srv, &code, req).await;
                        // notify 对账调度器（投影应用触发，§10.3 触发源 1）
                        coord.notify_one();
                        let _ = st;
                    });
                    tracing::info!(version, applied = ?outcome, "state applied + ack + notify");
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to parse state snapshot");
                },
            },
            "command" => match serde_json::from_str::<wakewake_protocol::CommandPayload>(&data) {
                Ok(payload) => {
                    // state-as-truth：command 通道只承载 Wol。execute async → spawn + complete。
                    let private_key = private_key.clone();
                    let state = state.clone();
                    let id_owned = id.clone();
                    let srv = server_url.clone();
                    let code = pairing_code.clone();
                    let hc = client.clone();
                    let wol_settings = settings.wol.clone();
                    tokio::spawn(async move {
                        let result = command_executor::execute(
                            &payload,
                            &private_key,
                            &state,
                            &wol_settings,
                        )
                        .await;
                        tracing::info!(
                            command_id = %id_owned,
                            success = result.success,
                            "command executed"
                        );
                        let _ = reporter::complete(&hc, &srv, &code, &id_owned, result).await;
                    });
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to parse command payload");
                },
            },
            other => {
                tracing::warn!(event = %other, "unknown SSE event type");
            },
        },
    }
}

/// §8.6 PEM 契约：把完整 SPKI PEM（含 `-----BEGIN/END-----` 标记 + 换行）剥成纯 base64 体。
/// agent 发送侧：去标记行 + 去所有换行 → 连续 base64 字符串（server 端重组）。
fn strip_pem_to_base64_body(pem: &str) -> String {
    const BEGIN: &str = "-----BEGIN PUBLIC KEY-----";
    const END: &str = "-----END PUBLIC KEY-----";
    pem.chars()
        .filter(|c| !matches!(c, '\n' | '\r'))
        .collect::<String>()
        .replace(BEGIN, "")
        .replace(END, "")
        .trim()
        .to_string()
}

/// 初始化 tracing：daily-rolling 文件 + stderr console。
fn init_tracing(settings: &Settings) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::layer::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, fmt};

    let log_dir = settings.log_dir();
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }
    let file_appender = tracing_appender::rolling::daily(&log_dir, "agent.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&settings.log.level));
    let file_layer = fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_ansi(false);
    let console_layer = if settings.log.format == "json" {
        fmt::layer().with_writer(std::io::stderr).boxed()
    } else {
        fmt::layer().with_writer(std::io::stderr).pretty().boxed()
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .init();
    guard
}
