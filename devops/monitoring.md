# Monitoring & observability (test / prod)

English | [中文](monitoring.zh.md)

Three systems, one signal each: **uptime-kuma** owns availability, **Beszel** owns host/container resources, **Grafana** (over the shared OTLP pipeline) owns service quality. Decisions and alternatives: the [OTLP wiring](../.agents/rfcs/implemented/process/2026-10-10-otlp-observability-wiring.md), [availability](../.agents/rfcs/implemented/process/2026-10-10-availability-monitoring.md), and [Beszel](../.agents/rfcs/implemented/process/2026-10-10-host-metrics-beszel.md) RFCs.

## Topology

```
wakewake@fn (test)     ──OTLP/gRPC LAN直连──►  otelcol @ NAS ──► VictoriaMetrics / VictoriaLogs / Jaeger ◄── Grafana @ NAS
wakewake@abj (prod)    ──OTLP/gRPC 127.0.0.1:4320 ──frp隧道──►  otelcol gRPC :4317 @ NAS
heartbeat/backup push  ──test: LAN直连 / prod: 127.0.0.1:3002──► uptime-kuma @ armbian
beszel agents          ──test: LAN直连 / prod: 127.0.0.1:8091──► beszel hub @ NAS
```

The observability stack (otelcol, stores, Grafana, Beszel hub, uptime-kuma) is operator-owned. Repo-owned pieces: the deploy wiring below, the heartbeat/backup-check producers, the Beszel agent compose, and the Grafana artifacts in `devops/grafana/`. The public edge for these services lives on the prod VPS itself (`*.bytehome.fun` → edge caddy → loopback frp ports → home); wakewake producers on that VPS use the loopback ports directly, bypassing the public edge by design.

## OTLP wiring (deploy side)

`group_vars/<env>/env.yml` sets `otel_otlp_endpoint` (test: `http://192.168.5.57:4317`; prod: `http://127.0.0.1:4320`); the vault holds `otel_otlp_token`. The compose template renders `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS` (bearer), and `OTEL_RESOURCE_ATTRIBUTES=deployment.environment.name=<env>` only when both exist. Metric/label shapes live in VictoriaMetrics: instruments keep their names (`http_request_duration_seconds` → `_bucket/_sum/_count`, `sse_connections_active`, counters with `_total`), resource attrs become labels (`deployment.environment.name`, `service.name=wakewake-server`, `scope.name`).

Token rotation is two-sided: add/replace `WW_OTLP_TOKEN_{TEST,PROD}` in the collector's env on the NAS and in the wakewake vaults, restart the collector, redeploy.

## Grafana

Repo artifacts: `devops/grafana/wakewake.yml` (dashboard provider), `devops/grafana/json/wakewake-runtime.json` (runtime dashboard, `$env` variable slices on `deployment.environment.name`), `devops/grafana/rules-wakewake.yaml` (managed alert rules).

Install/update onto the operator Grafana (NAS, `~/docker/grafana`):

```bash
scp devops/grafana/wakewake.yml           <nas>:docker/grafana/provisioning/dashboards/
scp devops/grafana/json/wakewake-runtime.json <nas>:docker/grafana/provisioning/dashboards/json/wakewake/
scp devops/grafana/rules-wakewake.yaml    <nas>:docker/grafana/provisioning/alerting/
ssh <nas> 'cd docker/grafana && docker compose restart grafana'
```

Alert rules (folder `wakewake`, prod-only): HTTP 5xx ratio > 5 % (5 m, warning) and > 20 % (2 m, critical); p95 latency > 1 s (10 m, warning); outbound mail blocked continuously 15 m (warning — kill switch or budget exhausted means registration/reset mail is not going out). Notification routing follows the operator Grafana's existing contact points and policies.

## uptime-kuma monitors

Create by hand in the kuma UI (operator instance). Naming: `wakewake · <env> · <object> (<layer>)`; one group per environment; common settings: heartbeat interval 60 s, retries 3, retry interval 60 s; push monitors: retries 2 (heartbeat) / 1 (backup).

| Monitor | Type | Target | Notes |
|---|---|---|---|
| wakewake · prod · homepage (edge) | HTTP(s) | `https://wakewake.online/` | expects 200; cert + domain-expiry notifications on |
| wakewake · prod · origin health | HTTP(s) JSON query | `https://wakewake.online/api/v1/health` | `$.status == ok` |
| wakewake · prod · host heartbeat (push) | Push, 120 s | push URL → vault `kuma_heartbeat_url` (prod) | producer: `heartbeat.py` |
| wakewake · prod · db backup (push) | Push, 86400 s | push URL → vault `kuma_backup_url` (prod) | producer: `backup-check.py` |
| wakewake · test · homepage | HTTP(s) | `https://192.168.5.200:8449/` | ignore TLS (self-signed) |
| wakewake · test · origin health | HTTP(s) JSON query | `https://192.168.5.200:8449/api/v1/health` | ignore TLS |
| wakewake · test · host heartbeat (push) | Push, 120 s | push URL → vault (test) | |
| wakewake · test · db backup (push) | Push, 86400 s | push URL → vault (test) | |

Push URL contract: producers need the **bare** endpoint (`…/api/push/<token>`, no query string) and append `?status=…&msg=…` themselves — a URL that already carries `?status=` makes kuma read arrays and flag every push as down.

### Heartbeat setup order (per environment)

1. Create the host-heartbeat push monitor in kuma; copy the **bare** push URL.
2. Vault it: `ansible-vault encrypt_string --vault-id wakewake-<env>@~/.local/bin/avpm-client --encrypt-vault-id wakewake-<env> '<url>' --name kuma_heartbeat_url >> devops/ansible/group_vars/<env>/vault.yml`
3. Deploy: `ansible-playbook devops/ansible/deploy.yml -l <env>` — the guarded tier installs `heartbeat.py`, the unit, enables linger, and starts pushing.
4. Verify: `systemctl --user status wakewake-heartbeat` and the kuma monitor turning green with a `msg` like `health + postgres ready`.

Removal (if ever needed): delete the vault var, delete the unit (`systemctl --user disable --now wakewake-heartbeat && rm ~/.config/systemd/user/wakewake-heartbeat.service`), remove `{{ app_path }}/heartbeat.py`, delete the kuma monitor.

## Beszel agents

Once per hub (operator instance, WebSocket-only mode): in the hub's **Settings → Tokens & Fingerprints**, enable a **permanent universal token** (or `GET /api/beszel/universal-token?enable=1&permanent=1` as a logged-in user), and copy the **public key** from the add-system dialog. There is no per-system token in 0.20+ — agents present the universal token and the hub auto-registers a system per agent fingerprint. Then, per environment:

1. `group_vars/<env>/env.yml`: `beszel_hub_url` is already set (test: `http://192.168.5.57:8090`; prod: `http://127.0.0.1:8091`); add `beszel_agent_key: "ssh-ed25519 AAAA…"` (public, not secret).
2. Vault the universal token: `… --name beszel_agent_token >> devops/ansible/group_vars/<env>/vault.yml`
3. Deploy. The guarded tier renders `{{ beszel_agent_path }}/docker-compose.yml`, pulls, and starts the agent.
4. First connect auto-registers a system named after the agent host — rename it to `wakewake-test` / `wakewake-prod` in the hub. Verify it shows live host + container stats within seconds.

Rotating the universal token is a hub-wide operation (it is bound to the hub user, one permanent token per user): mint the new one, re-vault in both environments, redeploy.

Thresholds in effect on the hub: disk 90 %, memory 92 %, CPU 95 % (each sustained 2 min), agent-down 1 min. Hub-side settings (users, notifications, the SQLite data dir's backup) are operator-owned.

## Alert triage

| Red signal | Likely failure | First action |
|---|---|---|
| edge homepage down, others green | edge/DNS/CF | check the edge caddy + DNS, then `curl` from outside |
| origin health down, heartbeat green | app-level (gateway routing, backend) | `docker logs wakewake`, `/api/v1/health` on the host loopback |
| heartbeat push down | process, DB, or host death | `systemctl --user status wakewake-heartbeat`, `docker compose ps`, `journalctl --user` |
| db backup push down | backup pipeline | see [backup.md](backup.md) failure modes |
| Grafana 5xx/p95 rules | service quality | runtime dashboard, then VictoriaMetrics queries by `deployment.environment.name` |
| Beszel disk/memory/cpu | host resources | hub system page for the per-container breakdown |
