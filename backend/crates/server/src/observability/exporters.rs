//! Local-file OpenTelemetry exporters (traces / metrics / logs).
//!
//! `opentelemetry-stdout` 0.32 hardcodes `println!` — there is no `with_writer`
//! plumbing on any of its three exporters. To write real OTel span/metric/log
//! data to local files (without an external OTLP collector) we implement the
//! three SDK exporter traits ourselves against official, public trait surfaces
//! and serialize via `serde_json` to a JSONL line per record.
//!
//! The writer is a `tracing_appender::non_blocking::NonBlocking` (which is
//! `Clone` + `impl std::io::Write`), sharing the same daily-rolling file
//! infrastructure as the `app`/`traces`/`metrics` fmt layers. Output is tagged
//! with `"otel": true` and `"signal": "trace"|"metric"|"log"` so it is
//! distinguishable from the tracing-fmt event lines that also land in those
//! files.
//!
//! Each exporter is only constructed when `OTEL_EXPORTER_OTLP_ENDPOINT` is unset
//! (local development); production keeps the OTLP exporters unchanged.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
use opentelemetry_sdk::logs::{LogBatch, LogExporter as SdkLogExporter};
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::trace::{SpanData, SpanExporter as SdkSpanExporter};

use tracing_appender::non_blocking::NonBlocking;

// ---------------------------------------------------------------------------
// Shared writer wrapper
// ---------------------------------------------------------------------------

/// A line-buffered JSONL writer shared by all three exporters. Each `write`
/// call serializes one `serde_json::Value` to a single line + newline and
/// hands it to the underlying `NonBlocking` writer. The `Mutex` serializes
/// access (exporters are called from the SDK's background threads).
#[derive(Debug)]
pub(crate) struct JsonlWriter {
    inner: Mutex<NonBlocking>,
}

impl JsonlWriter {
    pub fn new(writer: NonBlocking) -> Self {
        Self {
            inner: Mutex::new(writer),
        }
    }

    /// Serialize `value` as a single JSON line and write it. Errors are
    /// swallowed (non-blocking writer is best-effort; a failing disk shouldn't
    /// take the request path with it).
    fn write_line(&self, value: &serde_json::Value) {
        let mut buf = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(_) => return,
        };
        buf.push('\n');
        // NonBlocking's Writer is Write; ignore errors (queue full drops).
        let _ = self.inner.lock().map(|mut w| w.write_all(buf.as_bytes()));
    }
}

// ---------------------------------------------------------------------------
// Value conversion helpers (KeyValue / AnyValue -> serde_json::Value)
// ---------------------------------------------------------------------------

/// Convert a `KeyValue` pair into a `(String, Value)` for insertion into a JSON
/// object. Uses `Value`'s `Display` impl (opentelemetry implements it).
fn kv_to_json(kv: &KeyValue) -> (String, serde_json::Value) {
    (
        kv.key.as_str().to_string(),
        serde_json::Value::String(kv.value.to_string()),
    )
}

/// Convert an OTel `AnyValue` (log-record body/attribute values) to JSON.
fn any_value_to_json(v: &opentelemetry::logs::AnyValue) -> serde_json::Value {
    use opentelemetry::logs::AnyValue;
    match v {
        AnyValue::Int(i) => serde_json::json!(i),
        AnyValue::Double(d) => serde_json::json!(d),
        AnyValue::String(s) => serde_json::Value::String(s.as_str().to_string()),
        AnyValue::Boolean(b) => serde_json::json!(b),
        AnyValue::Bytes(b) => serde_json::Value::String(format!("{b:?}")),
        AnyValue::ListAny(items) => {
            serde_json::Value::Array(items.iter().map(any_value_to_json).collect())
        },
        AnyValue::Map(m) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in m.iter() {
                obj.insert(k.as_str().to_string(), any_value_to_json(val));
            }
            serde_json::Value::Object(obj)
        },
        // AnyValue is #[non_exhaustive]; fall back to Debug for any future
        // variant rather than failing to compile when the SDK adds one.
        _ => serde_json::Value::String(format!("{v:?}")),
    }
}

/// Format a `SystemTime` as an RFC 3339 string (or null if before epoch).
fn system_time_to_json(t: SystemTime) -> serde_json::Value {
    let dt = time::OffsetDateTime::from(t);
    serde_json::Value::String(
        dt.format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| format!("{dt:?}")),
    )
}

/// Duration in milliseconds between two `SystemTime`s (can be negative if
/// clocks are weird; we clamp to 0).
fn duration_ms(start: SystemTime, end: SystemTime) -> f64 {
    end.duration_since(start)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

// ---------------------------------------------------------------------------
// Trace exporter
// ---------------------------------------------------------------------------

/// OpenTelemetry `SpanExporter` that writes one JSON object per span to a
/// rolling file.
#[derive(Debug)]
pub struct FileSpanExporter {
    writer: JsonlWriter,
    resource: Resource,
}

impl FileSpanExporter {
    pub fn new(writer: NonBlocking) -> Self {
        Self {
            writer: JsonlWriter::new(writer),
            resource: Resource::builder().build(),
        }
    }
}

impl SdkSpanExporter for FileSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        for span in &batch {
            let mut attrs = serde_json::Map::new();
            for kv in &span.attributes {
                let (k, v) = kv_to_json(kv);
                attrs.insert(k, v);
            }
            let trace_id = span.span_context.trace_id().to_string();
            let span_id = span.span_context.span_id().to_string();
            let record = serde_json::json!({
                "otel": true,
                "signal": "trace",
                "trace_id": trace_id,
                "span_id": span_id,
                "parent_span_id": span.parent_span_id.to_string(),
                "parent_span_is_remote": span.parent_span_is_remote,
                "name": span.name,
                "kind": format!("{:?}", span.span_kind),
                "start_time": system_time_to_json(span.start_time),
                "end_time": system_time_to_json(span.end_time),
                "duration_ms": duration_ms(span.start_time, span.end_time),
                "status": format!("{:?}", span.status),
                "attributes": attrs,
                "instrumentation_scope": span.instrumentation_scope.name(),
                "resource": resource_to_json(&self.resource),
            });
            self.writer.write_line(&record);
        }
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn set_resource(&mut self, res: &Resource) {
        self.resource = res.clone();
    }
}

// ---------------------------------------------------------------------------
// Metric exporter
// ---------------------------------------------------------------------------

/// OpenTelemetry `PushMetricExporter` that writes aggregated metric snapshots
/// (one JSON object per metric) to a rolling file. Driven by
/// `PeriodicReader` (default 60s).
#[derive(Debug)]
pub struct FileMetricExporter {
    writer: JsonlWriter,
    resource: Resource,
    is_shutdown: std::sync::atomic::AtomicBool,
}

impl FileMetricExporter {
    pub fn new(writer: NonBlocking) -> Self {
        Self {
            writer: JsonlWriter::new(writer),
            resource: Resource::builder().build(),
            is_shutdown: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl PushMetricExporter for FileMetricExporter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        if self.is_shutdown.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(OTelSdkError::AlreadyShutdown);
        }
        let now = system_time_to_json(SystemTime::now());
        for scope_metrics in metrics.scope_metrics() {
            for metric in scope_metrics.metrics() {
                let record = serialize_metric(metric, &now, &self.resource);
                self.writer.write_line(&record);
            }
        }
        Ok(())
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        self.is_shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}

/// Serialize one `Metric` (and its aggregation) into a JSON object.
fn serialize_metric(
    metric: &opentelemetry_sdk::metrics::data::Metric,
    now: &serde_json::Value,
    resource: &Resource,
) -> serde_json::Value {
    let data = metric.data();
    // Extract data_points depending on numeric type + aggregation kind.
    let (kind, body) = match data {
        AggregatedMetrics::U64(md) => metric_data_body(md),
        AggregatedMetrics::I64(md) => metric_data_body(md),
        AggregatedMetrics::F64(md) => metric_data_body(md),
    };
    serde_json::json!({
        "otel": true,
        "signal": "metric",
        "timestamp": now,
        "name": metric.name(),
        "description": metric.description(),
        "unit": metric.unit(),
        "kind": kind,
        "data_points": body,
        "resource": resource_to_json(resource),
    })
}

/// Produce `(kind_str, data_points_json)` for a `MetricData<T>`. `T` is erased
/// here by a small macro because the SDK boxes the numeric type behind
/// `AggregatedMetrics`.
fn metric_data_body<T: Copy + std::fmt::Debug>(
    md: &MetricData<T>,
) -> (&'static str, serde_json::Value) {
    match md {
        MetricData::Gauge(g) => {
            let dps: Vec<_> = g
                .data_points()
                .map(|dp| {
                    let mut attrs = serde_json::Map::new();
                    for kv in dp.attributes() {
                        let (k, v) = kv_to_json(kv);
                        attrs.insert(k, v);
                    }
                    serde_json::json!({
                        "attributes": attrs,
                        "value": format!("{:?}", dp.value()),
                    })
                })
                .collect();
            ("gauge", serde_json::Value::Array(dps))
        },
        MetricData::Sum(s) => {
            let dps: Vec<_> = s
                .data_points()
                .map(|dp| {
                    let mut attrs = serde_json::Map::new();
                    for kv in dp.attributes() {
                        let (k, v) = kv_to_json(kv);
                        attrs.insert(k, v);
                    }
                    serde_json::json!({
                        "attributes": attrs,
                        "value": format!("{:?}", dp.value()),
                    })
                })
                .collect();
            (
                if s.is_monotonic() { "counter" } else { "sum" },
                serde_json::json!({
                    "temporality": format!("{:?}", s.temporality()),
                    "is_monotonic": s.is_monotonic(),
                    "points": dps,
                }),
            )
        },
        MetricData::Histogram(h) => {
            let dps: Vec<_> = h
                .data_points()
                .map(|dp| {
                    let mut attrs = serde_json::Map::new();
                    for kv in dp.attributes() {
                        let (k, v) = kv_to_json(kv);
                        attrs.insert(k, v);
                    }
                    let bounds: Vec<_> = dp.bounds().collect();
                    let counts: Vec<_> = dp.bucket_counts().map(|c| serde_json::json!(c)).collect();
                    serde_json::json!({
                        "attributes": attrs,
                        "count": dp.count(),
                        "sum": format!("{:?}", dp.sum()),
                        "min": dp.min().map(|v| format!("{v:?}")),
                        "max": dp.max().map(|v| format!("{v:?}")),
                        "bounds": bounds,
                        "bucket_counts": counts,
                    })
                })
                .collect();
            (
                "histogram",
                serde_json::json!({
                    "temporality": format!("{:?}", h.temporality()),
                    "points": dps,
                }),
            )
        },
        MetricData::ExponentialHistogram(_) => ("exponential_histogram", serde_json::json!([])),
    }
}

// ---------------------------------------------------------------------------
// Log exporter
// ---------------------------------------------------------------------------

/// OpenTelemetry `LogExporter` that writes one JSON object per log record to a
/// rolling file.
#[derive(Debug)]
pub struct FileLogExporter {
    writer: JsonlWriter,
    resource: Resource,
}

impl FileLogExporter {
    pub fn new(writer: NonBlocking) -> Self {
        Self {
            writer: JsonlWriter::new(writer),
            resource: Resource::builder().build(),
        }
    }
}

impl SdkLogExporter for FileLogExporter {
    async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
        for (record, _scope) in batch.iter() {
            let mut attrs = serde_json::Map::new();
            for (k, v) in record.attributes_iter() {
                attrs.insert(k.as_str().to_string(), any_value_to_json(v));
            }
            let (trace_id, span_id) = match record.trace_context() {
                Some(tc) => (tc.trace_id.to_string(), Some(tc.span_id.to_string())),
                None => (String::new(), None),
            };
            let body = record
                .body()
                .map_or(serde_json::Value::Null, any_value_to_json);
            let rec = serde_json::json!({
                "otel": true,
                "signal": "log",
                "timestamp": record.timestamp().map_or(serde_json::Value::Null, system_time_to_json),
                "observed_timestamp": record.observed_timestamp().map_or(serde_json::Value::Null, system_time_to_json),
                "severity": record.severity_number().map(|s| format!("{s:?}")),
                "severity_text": record.severity_text(),
                "target": record.target().map(std::string::ToString::to_string),
                "body": body,
                "attributes": attrs,
                "trace_id": trace_id,
                "span_id": span_id,
                "resource": resource_to_json(&self.resource),
            });
            self.writer.write_line(&rec);
        }
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn set_resource(&mut self, res: &Resource) {
        self.resource = res.clone();
    }
}

// ---------------------------------------------------------------------------
// Resource helper
// ---------------------------------------------------------------------------

fn resource_to_json(resource: &Resource) -> serde_json::Value {
    // `Resource::iter()` yields `(&Key, &Value)` in 0.32 (a HashMap iterator).
    let mut obj = serde_json::Map::new();
    for (k, v) in resource {
        obj.insert(
            k.as_str().to_string(),
            serde_json::Value::String(v.to_string()),
        );
    }
    serde_json::Value::Object(obj)
}
