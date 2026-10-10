# RFC: Availability monitoring — uptime-kuma push producers and the monitor inventory

Status: implemented

English | [中文](2026-10-10-availability-monitoring.zh.md)

## Problem

A deployed wakewake environment can fail in at least four ways an outside observer cares about: the public edge is unreachable (DNS, Cloudflare, the host gateway), the origin is up but serving errors, the process or its database is wedged while the edge still looks fine, or the whole host is dead. Nothing watched any of these: a user discovering the service is down via their alarm clock not going off is the current detection mechanism.

The operator already runs an uptime-kuma instance on the home LAN (reachable in-network and through the public edge), so the decision here is not which monitor to install but how wakewake's part is owned and automated without reaching into the operator's instance.

## Decision

The kuma instance stays operator-owned; the repo owns the **producers** (processes that push verdicts) and the **inventory** (the authoritative list of monitors to create), and the two halves meet at vaulted push URLs.

- **Heartbeat producer** — `devops/ansible/files/heartbeat.py` runs on each monitored host as a systemd user unit (`wakewake-heartbeat.service`, linger, `Restart=always`): every 60 s it probes the host health URL (`heartbeat_health_url` per environment; body must say `ok`; self-signed environments use `--insecure`) *and* `pg_isready` through `docker compose exec` against the exact socket path the app uses — `/api/v1/health` is deliberately liveness-only so Docker healthchecks don't flap on DB load, so the heartbeat adds the DB probe itself. The verdict is pushed to kuma.
- **Dual failure channels** — each push monitor goes down either on an explicit `status=down` push (app-level failure, which covers the edge blind spot since the push path bypasses the public ingress) or on silence (host death, script crash). The push URL is a secret — anyone holding it can forge `up` beats and mask an outage — so it arrives only via `KUMA_HEARTBEAT_URL` in the unit (0600, vault-rendered), never in the script; the backup tier's verdicts ride the same contract through `KUMA_BACKUP_URL` in `backup.env`.
- **Bare push URL contract** — producers append `?status=…&msg=…` to a query-less URL. A second `?status=` suffix (the form kuma's copy-paste offers) makes kuma read the status as an array and flag the monitor down on every push; the runbook carries the bare-URL form only.
- **Push paths per environment** — test pushes LAN-direct to the kuma instance; prod pushes to a loopback port on its own VPS that the operator's edge tunnels home (the same pattern as the OTLP ingress). Neither producer traverses the public edge.
- **Inventory, not scripting** — the monitors (edge homepage, origin health JSON query, host heartbeat push, db backup push; per environment, kuma group per environment) are documented field-by-field in [devops/monitoring.md](../../../../devops/monitoring.md) and created by hand once. Alert delivery (notification channels, retry policy) is kuma-side configuration owned by the operator.

## Verification

On both environments: the heartbeat unit runs under linger, `systemctl --user status wakewake-heartbeat` shows continuous pushes, the kuma push monitors report up with live `msg` payloads, and stopping the postgres container flips the heartbeat to `status=down` postgres-probe messages within one interval. The setup-order runbook (create monitor → vault the URL → deploy → verify) was executed as written.

## Alternatives considered

**Scripting kuma monitor creation via its API.** Lost: a new dependency against an unversioned private API to configure a handful of monitors exactly once; the API surface is a compatibility risk that outlives the one-time setup, and the runbook's field table is reviewable in a PR while code that mutates someone else's monitoring instance is not.

**Poll-only monitoring (kuma dials the public URL, no producers).** Lost: it sees the edge, not the origin — a wedged process behind a healthy gateway, or a dead database behind a healthy process, both look green. The reverse-push channel is the only vantage that survives gateway and DNS failure.

**Grafana alerting for availability.** Lost: the telemetry pipeline has no always-on series (instruments first appear on first increment), so an absence-based alert there is noisy by construction; availability is a push contract, not a query.

**A heavier monitor stack (Pingdom/UptimeRobot/Better Stack).** Lost: the operator instance already exists, is self-hosted, and its notification wiring is already tuned; a SaaS adds accounts, quotas, and a third party seeing every probe target for zero additional coverage.

## Consequences

- Availability verdicts arrive from inside each environment over paths that bypass the public edge, so a full edge outage is distinguishable from a full host outage (edge monitor red + heartbeat green vs. both red).
- The heartbeat widens the health signal beyond the app: a dead postgres turns the monitor down even though `/api/v1/health` stays `ok` — deliberate, and the monitor's `msg` names which probe failed.
- Rotating a push URL means recreating the kuma monitor, re-vaulting, and redeploying — three steps the runbook sequences; losing a URL to anyone is a forged-`up` risk, so URLs are treated as secrets of the same class as tokens.
