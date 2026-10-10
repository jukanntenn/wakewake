# RFC: Host metrics with Beszel agents on the monitored hosts

Status: implemented

English | [中文](2026-10-10-host-metrics-beszel.zh.md)

## Problem

Everything above the application layer is invisible: disk filling up under postgres volumes and container logs, memory pressure on a small VPS swapping into the ground, CPU saturation during image pulls, a container restart-looping under `restart: always` with the health endpoint none the wiser. The application telemetry (OTLP) and the availability pushes (kuma) both live and die with the host — none of them can report the host itself dying. Container-level visibility matters too, since the app and its postgres sibling are the only long-running containers whose resource regression would be actionable.

## Decision

Each monitored host runs a Beszel agent; the hub is operator-owned on the observability server, and the repo owns exactly the agent half.

- **Agent deployment** — `templates/beszel-agent-compose.yml.j2` renders a one-service compose project (`{{ beszel_agent_path }}`, a sibling of the app): `network_mode: host` (host counters read directly; the only traffic is one outbound WebSocket), `/var/run/docker.sock` mounted read-only (container stats; read-only limits mutation, not visibility), pinned image (`henrygd/beszel-agent:0.20.0` — 0.19+ agents verify the hub's TLS certificate, so the pin is a floor), `KEY` set to the hub's public key (not a secret — WebSocket mode uses it to verify the hub's signed challenge), and `TOKEN` to the hub's **universal registration token**, vaulted as `beszel_agent_token`. The deploy tasks require hub URL, key, and token together (atomic credential pair: the agent never ships half-configured) and never uninstall — removal is a documented manual step. Since Beszel 0.20 there is no per-system token: the agent presents the universal token, and the hub find-or-creates the system record from the agent's fingerprint on first connect — deployment is followed by a one-time rename of the auto-registered system to `wakewake-<env>`.
- **Hub addressing** — test agents reach the hub LAN-direct; the prod agent reaches it through a loopback port on its own VPS tunneled home by the operator's edge (the same pattern the OTLP ingress and the kuma push path use). The hub runs WebSocket-only (`DISABLE_SSH`): agents dial out, the hub never dials in, so nothing inbound is exposed on any monitored host.
- **Ownership boundary** — the hub (its SQLite data, users, thresholds, notifications) is the operator's; the repo's runbook in [devops/monitoring.md](../../../../devops/monitoring.md) records the add-system steps, the alert thresholds in effect (disk 90 %, memory 92 %, CPU 95 %, sustained 2 min; agent-down 1 min), and where the boundary sits: Beszel owns *host and container* resource alerts, kuma owns *availability*, Grafana owns *service quality* — one signal, one owner, no double-paging.

## Verification

On both environments: the agent compose project came up under the playbook's guarded tasks, the containers report `running`, and the hub shows both systems live with host and per-container stats within seconds of the agent start. Deploying with the vault vars absent left the tier skipped and the playbook green.

## Alternatives considered

**Netdata on each host.** Lost: 250–350 MB resident per host against a monitoring budget of roughly 128–256 MiB for the agent tier; Beszel's agent is a single small Go binary. Netdata's depth (per-process, eBPF) is diagnostic overkill for "is the box healthy".

**A Prometheus-shaped stack (node_exporter + cadvisor + scraper).** Lost: three more per-host moving parts plus a scraper, storage, and retention somewhere — re-litigating the push-vs-scrape question the observability wiring already settled, on hosts where the answer was "no new daemons".

**Relying on OTLP-only host metrics.** Lost: the host metrics would share fate with the application's telemetry pipeline (collector outage = blind), and the Rust host-instrument ecosystem is immature compared to a purpose-built agent that keeps reporting through application chaos.

**Extending uptime-kuma with resource monitors.** Lost: kuma's resource monitoring (where present) is instance-scoped and carries the same shared-fate problem, with no per-container breakdown.

**Co-locating a hub per project.** Lost: N hubs, N sets of users and notifications, and the monitoring tier's whole point — independence from the monitored systems — diluted into per-project snowflakes.

## Consequences

- Host and container resource alerts for every monitored wakewake host come from a system that survives the host's application stack failing entirely.
- The agent reads the Docker socket (read-only): it can see every container on shared hosts, which is acceptable on single-operator machines and worth remembering before pointing it at multi-tenant hosts.
- Agent version drift is manual: the pin sits in per-env vars, and upgrading means changing the pin and re-running the playbook; the hub tolerates mixed agent versions within a minor series.
- A lost hub means losing alert *history* (thresholds and systems are re-creatable — the runbook lists them); the hub's data dir is the operator's backup responsibility, recorded in the runbook.
