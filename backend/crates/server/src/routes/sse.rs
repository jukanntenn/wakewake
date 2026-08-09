//! SSE 路由（device-sync-v3 §8.6）。Agent M2M 域（pairing-code）。
//!
//! GET /agents/self/events：
//! - 鉴权（pairing-code 中间件已注入 `AuthAgent`）
//! - X-Public-Key 头逻辑：agent 发送**纯 base64 体**（剥 PEM 标记 + 换行），
//!   server 调 `reassemble_spki_pem` 重组标准 SPKI PEM（§8.6 契约）；写/替换/跳过；更新 last_seen
//! - 注册 Hub receiver（subscribe 初始化 last_pushed=last_acked=握手快照 V，§5.4.1）
//! - 推 : connected comment + event: state（全量快照 + version + aid）
//! - state 事件**省略 `id:`**（§8.6：不实现 SSE resume）；command 事件保留 `id: c_...`
//! - 心跳 : ping 每 30s（覆盖 CF 120s timeout）

use std::convert::Infallible;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use futures_util::StreamExt;
use futures_util::stream::{self, Stream};
use tokio_stream::wrappers::ReceiverStream;

use crate::error::AppResult;
use crate::hub::{CommandDispatcher, SseEvent};
use crate::middleware::agent_auth::AuthAgent;
use crate::repo::agent_repo;
use crate::service::state as state_service;
use crate::state::AppState;

pub fn routes() -> Router<std::sync::Arc<AppState>> {
    Router::new().route("/agents/self/events", get(events))
}

async fn events(
    State(state): State<std::sync::Arc<AppState>>,
    agent: AuthAgent,
    headers: HeaderMap,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // X-Public-Key 逻辑（§8.6 PEM 契约）：agent 发送纯 base64 体，server 重组标准 SPKI PEM。
    if let Some(pk) = headers.get("X-Public-Key").and_then(|v| v.to_str().ok()) {
        let current = agent_repo::find_by_id(&state.pool, agent.agent_id)
            .await
            .map_err(crate::error::AppError::from_repo)?
            .and_then(|a| a.public_key);
        if current.as_deref() != Some(pk) {
            agent_repo::upsert_public_key(&state.pool, agent.agent_id, Some(pk))
                .await
                .map_err(crate::error::AppError::from_repo)?;
        }
    } else {
        agent_repo::touch_last_seen(&state.pool, agent.agent_id)
            .await
            .map_err(crate::error::AppError::from_repo)?;
    }

    // 推全量 state 快照（含 version + aid，§8.6）
    let snapshot = state_service::build_for_agent(&state.pool, agent.agent_id).await?;
    let state_json = serde_json::to_string(&snapshot).unwrap_or_default();
    let initial_version = i64::try_from(snapshot.version).unwrap_or(0);

    // 订阅 Hub 命令 channel（SSE 连接在 → online）。
    // §5.4.1 关键：subscribe 初始化 last_pushed = last_acked = initial_version（握手快照 V）。
    let rx = state.hub.subscribe(agent.agent_id, initial_version);
    crate::observability::metrics::record_sse_connections(state.hub.connected_count() as u64);
    tracing::info!(
        agent_id = agent.agent_id,
        user_id = agent.user_id,
        has_public_key = headers.contains_key("X-Public-Key"),
        initial_version,
        "SSE agent connected (online)"
    );

    // 事件流：Hub 的 SseEvent 分流为 `event: command` 或 `event: state`。
    let event_stream = ReceiverStream::new(rx).filter_map(|sse_event| match sse_event {
        SseEvent::Command(cmd) => {
            let id = cmd.id.clone();
            let payload = serde_json::to_string(&cmd.payload).unwrap_or_default();
            std::future::ready(Some(Ok(Event::default()
                .event("command")
                .id(id) // command 事件保留 id（agent 用于 complete 回报）
                .data(payload))))
        },
        SseEvent::State { snapshot, .. } => {
            let snapshot_json = serde_json::to_string(&snapshot).unwrap_or_default();
            std::future::ready(Some(Ok(
                // §8.6：state 事件省略 id（不实现 SSE resume）
                Event::default().event("state").data(snapshot_json),
            )))
        },
    });

    // 首条 : connected comment + 初始 state event + 后续事件流
    let inner_stream =
        stream::once(async { Ok::<_, Infallible>(Event::default().comment("connected")) })
            .chain(stream::once(async move {
                // §8.6：握手首推 state，省略 id
                Ok::<_, Infallible>(Event::default().event("state").data(state_json))
            }))
            .chain(event_stream);

    // 断开时注销 Hub（agent → offline）。
    let hub = state.hub.clone();
    let agent_id = agent.agent_id;
    let stream = CleanupStream::new(inner_stream, move || {
        hub.unsubscribe(agent_id);
        crate::observability::metrics::record_sse_connections(hub.connected_count() as u64);
        tracing::info!(agent_id, "SSE agent disconnected (offline)");
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("ping"),
    ))
}

/// 包装流，在流被 drop 时执行 cleanup（注销 Hub，agent → offline）。
struct CleanupStream<F: FnOnce()> {
    inner: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>,
    cleanup: Option<F>,
}

impl<F: FnOnce()> Unpin for CleanupStream<F> {}

impl<F: FnOnce()> CleanupStream<F> {
    fn new<S>(inner: S, cleanup: F) -> Self
    where
        S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
    {
        Self {
            inner: Box::pin(inner),
            cleanup: Some(cleanup),
        }
    }
}

impl<F: FnOnce()> Drop for CleanupStream<F> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

impl<F: FnOnce()> Stream for CleanupStream<F> {
    type Item = Result<Event, Infallible>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.inner.as_mut().poll_next(cx)
    }
}
