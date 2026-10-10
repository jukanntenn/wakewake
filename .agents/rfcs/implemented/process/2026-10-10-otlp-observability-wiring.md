# RFC: OTLP observability wiring — push telemetry to the operator stack, dashboards and alerts as repo artifacts

Status: implemented

English | [中文](2026-10-10-otlp-observability-wiring.zh.md)

## Problem

The backend already speaks OpenTelemetry: `observability` initializes traces/logs/metrics with an OTLP exporter when `OTEL_EXPORTER_OTLP_ENDPOINT` is set and falls back to local JSONL files otherwise, and `observability::metrics` defines the full instrument inventory. But no deployment ever set the endpoint, so every environment runs in file mode: telemetry exists as rotating files on each host that nobody reads, there is no cross-environment view, no retention beyond disk, and no alerting on service quality. Meanwhile an operator-owned observability stack (a shared collector fanning out to Jaeger, VictoriaMetrics, and VictoriaLogs, fronted by Grafana) already runs on the home NAS and is reachable from every environment — one OTLP front door with bearer-token auth per producer.

One gap surfaced while wiring this: the `http_request_duration_seconds` histogram was declared but nothing recorded it, so any HTTP panel or alert would have queried a metric that never exists.

## Decision

Deployment wiring only — the backend keeps its existing OTLP dual mode, and the deploy layer turns it on per environment; no new dependencies.

- **Env injection** — `templates/docker-compose.yml.j2` renders three container env vars, guarded on both `otel_otlp_endpoint` (per-env var) and `otel_otlp_token` (vault) being defined: `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS=Authorization=Bearer%20<token>` (the tonic exporter reads this env var itself and URL-decodes the value), and `OTEL_RESOURCE_ATTRIBUTES=deployment.environment.name=<env>`. The resource attribute rides the SDK's env detector and lands in the store as the `deployment.environment.name` label — the single slice key every dashboard and alert uses. Unguarded environments render byte-identically to before and keep the file-mode fallback.
- **Paths** — test pushes OTLP/gRPC LAN-direct to the collector (`:4317`); prod pushes over a dedicated frp tunnel that maps a loopback port on the prod VPS (`127.0.0.1:4320`) to the collector's gRPC port on the NAS. The tunnel rides the same frps the operator's other services use, as a separate user-owned compose project (`frpc-wakewake`) so it neither touches the operator's main tunnel client nor needs root. The collector authenticates every producer by bearer token; wakewake has its own per-environment tokens, added to the collector's token list as `WW_OTLP_TOKEN_{TEST,PROD}`.
- **The missing recorder** — `middleware/http_metrics.rs` now records `http_request_duration_seconds` (elapsed, method, route template, status), attached via `route_layer` so `MatchedPath` is available: the `path` label is the route template (`/api/v1/auth/login`), never the raw URI, keeping series cardinality bounded. It sits inside the global rate-limit layer; governor 429s are counted by `http_rate_limited_total` instead, and health routes stay uninstrumented by design.
- **Grafana artifacts in the repo** — `devops/grafana/` carries the runtime dashboard JSON (request rate/5xx ratio/p95, SSE connections, commands, wakes, mail, rate-limit, purges — sliced by `$env`), the dashboard provider (`wakewake.yml`), and the managed alert rules (`rules-wakewake.yaml`: 5xx ratio warn/crit, p95 latency, sustained mail-blocking; prod-only, since test is a fast-iteration environment where red is normal). The repo is the source; installing them onto the operator Grafana's provisioning directories is a documented two-file copy plus restart in [devops/monitoring.md](../../../../devops/monitoring.md). Queries were written against the metric names and labels observed live in VictoriaMetrics after the first wired deployment, not against assumptions.

## Verification

On the test environment after wiring: VictoriaMetrics received `wakewake-server`-scoped series with `deployment.environment.name=test`; a 30-request burst produced exactly matching histogram counts (`10×422 + 20×429` by route-template path) and `http_rate_limited_total`; the gRPC path was probed directly (unauthenticated export → `Unauthenticated`, authenticated empty export → accepted). The dashboard and alert files loaded into the operator Grafana without errors, and the alert rules entered the scheduler.

## Alternatives considered

**A Prometheus scrape endpoint (`/metrics`).** Lost: it needs a new crate (`opentelemetry-prometheus` or `prometheus`) plus an exposed port and a scraper with storage — new moving parts on a 2 GB VPS and a second metrics vocabulary — while the push pipeline, the collector, and the store already exist and the backend already links the exporter.

**File mode forever (do nothing).** Lost: telemetry that requires SSH and eyeballs per host, no cross-environment slicing, no alerting, and logs/traces/metrics on the same disk as the service — the failure modes you most want evidence for are the ones that take the evidence with them.

**Grafana Cloud or another SaaS.** Lost: the operator stack is already paid for and running; SaaS adds an external dependency, per-host egress to a third party, and per-user cost for a personal tool.

**Riding the existing HTTP tunnel for prod instead of a new gRPC tunnel.** Lost: the backend's exporter is tonic (gRPC); reusing the HTTP-only ingress would mean switching exporter protocols in shipped source and re-validating the dependency matrix, to save one loopback port. The tunnel is operator-side config, not application code.

**Hand-built dashboards in the Grafana UI (no repo artifacts).** Lost: UI-built dashboards are unreproducible — lose the Grafana volume and the operational picture is gone; the next environment wiring starts from memory. As provisioned files they review like code and reinstall in one copy.

## Consequences

- Every wired environment emits traces, metrics, and logs to the shared stack within one export cycle (60 s); environments distinguish themselves solely by `deployment.environment.name`.
- Prod telemetry depends on the frp tunnel staying up: a tunnel outage degrades to the local JSONL files (the dual-mode fallback is unchanged) and shows up as stale series, not lost data. Tokens rotate collector-side and vault-side together, or exports 401 silently.
- There is no always-on heartbeat metric in the pipeline (instruments first appear on first increment), so telemetry-gap alerting stays out of Grafana; availability is owned by the kuma push monitors in [availability monitoring](2026-10-10-availability-monitoring.md).
- HTTP duration now has exactly one recorder; a future second instrumentation point must reuse `record_http_duration` rather than touching the histogram directly.
