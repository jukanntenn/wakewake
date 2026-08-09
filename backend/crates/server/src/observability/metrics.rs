//! wakewake 业务指标（OTel）。
//!
//! 移植自 grill-me-sleek `observability/metrics.rs` 框架，指标按 wakewake 业务替换。
//! 通过 [`metrics()`] 全局访问；`init_metrics()` 在 server 启动时（tracing 之后）调用。
//! 本地开发经 [`super::exporters::FileMetricExporter`] 写 `metrics.*` 文件；生产走 OTLP。

use opentelemetry::metrics::{Counter, Gauge, Histogram};
use std::sync::OnceLock;

/// 业务指标集。名称无前缀（单服务，OTel/Prom 惯例）。
pub struct Metrics {
    pub sse_connections_active: Gauge<u64>,
    pub commands_dispatched_total: Counter<u64>,
    pub commands_completed_total: Counter<u64>,
    pub wake_total: Counter<u64>,
    pub http_request_duration_seconds: Histogram<f64>,
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

/// 从全局 meter provider 创建业务指标。在 [`super::init_tracing`] 之后调用。
pub fn init_metrics() {
    let meter = opentelemetry::global::meter("wakewake-server");

    let sse_connections_active = meter
        .u64_gauge("sse_connections_active")
        .with_description("Current online agent SSE connections")
        .build();

    let commands_dispatched_total = meter
        .u64_counter("commands_dispatched_total")
        .with_description("Total commands dispatched to agents")
        .build();

    let commands_completed_total = meter
        .u64_counter("commands_completed_total")
        .with_description("Total command completions received from agents")
        .build();

    let wake_total = meter
        .u64_counter("wake_total")
        .with_description("Total wake records written")
        .build();

    let http_request_duration_seconds = meter
        .f64_histogram("http_request_duration_seconds")
        .with_description("HTTP request processing duration in seconds")
        .build();

    let _ = METRICS.set(Metrics {
        sse_connections_active,
        commands_dispatched_total,
        commands_completed_total,
        wake_total,
        http_request_duration_seconds,
    });
}

/// Get the global metrics instance.
pub fn metrics() -> Option<&'static Metrics> {
    METRICS.get()
}

// ---------------------------------------------------------------------------
// Domain-specific helpers
// ---------------------------------------------------------------------------

/// Record the current online agent SSE connection count.
pub fn record_sse_connections(count: u64) {
    if let Some(m) = metrics() {
        m.sse_connections_active.record(count, &[]);
    }
}

/// Record a command dispatched to an agent.
pub fn record_command_dispatched() {
    if let Some(m) = metrics() {
        m.commands_dispatched_total.add(1, &[]);
    }
}

/// Record a command completion received from an agent.
pub fn record_command_completed(success: bool) {
    if let Some(m) = metrics() {
        m.commands_completed_total.add(
            1,
            &[opentelemetry::KeyValue::new(
                "status",
                if success { "success" } else { "failed" },
            )],
        );
    }
}

/// Record a wake record written to the audit table.
pub fn record_wake(status: &str) {
    if let Some(m) = metrics() {
        m.wake_total.add(
            1,
            &[opentelemetry::KeyValue::new("status", status.to_string())],
        );
    }
}

/// Record HTTP request duration.
pub fn record_http_duration(elapsed: f64, method: &str, path: &str, status: u16) {
    if let Some(m) = metrics() {
        m.http_request_duration_seconds.record(
            elapsed,
            &[
                opentelemetry::KeyValue::new("method", method.to_string()),
                opentelemetry::KeyValue::new("path", path.to_string()),
                opentelemetry::KeyValue::new("status", status.to_string()),
            ],
        );
    }
}
