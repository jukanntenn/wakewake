# Load and capacity testing spec (prod-sim, scenario matrix, judgement methodology)

English | [中文](load.zh.md)

> This document is the single specification for wakewake load testing: the two test rigs, the production-fidelity environment contract, the scenario matrix, the data contract of the seed tool, the red lines, and the methodology that turns measurements into a sweet-spot / capacity-extremes table. Operational runbook details live beside the scripts in `tests/load/`; decision history lives in [`.agents/rfcs/`](../../.agents/rfcs/README.md).

## 1. Two rigs, two questions

| Rig | Entry | Environment | Answers |
|---|---|---|---|
| Bare-metal | `tests/load/run.sh` | e2e compose + `docker-compose.load.yml`: unlimited CPU/RAM, load-tuned PG (512MB buffers, 200 connections), rate limits disabled | Per-connection memory slope and linearity (the 100k extrapolation) |
| prod-sim | `tests/load/run-prod-sim.sh` | e2e compose + `docker-compose.prod-sim.yml`: 2c/2g resource split, production PG GUCs, 3Mbps egress shaping, rate limits **on** | Sweet spot and capacity extremes of the real deployment target (2c/2g/3Mbps VPS behind free Cloudflare) |

The bare rig measures what does not depend on hardware (memory per connection); the prod-sim rig measures everything that does (CPU ceilings, bandwidth walls, rate-limit calibration). A number quoted for production capacity must come from prod-sim.

## 2. prod-sim environment contract

```
app:      cpuset "0,1" (LOAD_CPUSET), mem_limit/memswap 1400m, nofile 1M, production image (s6: Caddy + backend)
postgres: cpuset "0,1", mem_limit 550m, GUCs shared_buffers=256MB max_connections=50 effective_cache_size=1GB maintenance_work_mem=128MB synchronous_commit=off
egress:   tc tbf rate 3mbit burst 32kbit latency 400ms (in the app netns, injected by the wan-shaper sidecar; optional netem delay on top)
network:  wakewake-load-net bridge — k6 joins the bridge, never --network=host (host networking bypasses the shaping netns)
frontend: plain-HTTP Caddyfile (docker/Caddyfile) mounted over the e2e TLS variant
limits:   WAKEWAKE_RATE_LIMIT__DISABLED=false by default; LOAD_DISABLE_RATE_LIMIT=1 is an explicit, result-annotating escape hatch for hardware-ceiling probes
PG port:  published on host 15432 (5432 is commonly taken by the dev compose) for seed access
```

Invariants: the app container restart recreates the netns, so the orchestrator re-injects shaping after every restart (R3 depends on this); helper images (`wakewake-k6-sse:1.2.1` = k6 v1.2.1 + xk6-sse v0.1.12, `wakewake-wan-shaper` = alpine + iproute2) are built locally from `tests/load/*.Dockerfile` instead of modifying the production image.

Known fidelity losses (accepted, listed here so results are read correctly): no TLS termination cost (production terminates TLS at Cloudflare; the origin cert handshake is the only missing CPU slice); loader and SUT share one host (compensate with `WAN_DELAY`, e.g. 40ms); the Cloudflare edge is not simulated — origin-direct equals the all-cache-miss worst case; CFS quota is not pin-identical to two real cores (pin with `cpuset` on the dev box for tighter numbers).

## 3. Scenario matrix

| ID | File | Shape | Answers | Gate |
|---|---|---|---|---|
| R1 | `r1-sse-knee.js` | staircase VUs → `TARGET_CONNS` by `STAIR_STEP`, 2m ramp + 2m hold per level | SSE connection knee (memory/fd/CPU) | connect p95 < 1s; slope red lines §6 |
| R2 | `r2-login-staircase.js` | arrival-rate staircase 1→16 logins/s, 2m per level (**limits off**) | bcrypt CPU ceiling (replaces the ×2.5 projection) | knee = last level with p99 < 1.5s, errors < 0.1% |
| R3 | `r3-reconnect-storm.js` | `CONNS` held connections; orchestrator `docker restart` at `R3_RESTART_AFTER`; VUs replay the agent backoff algorithm | storm convergence, 3Mbps saturation duration, DB amplification | failures zero after the restart window; reconnect p99 < 60s |
| R4 | `r4-dashboard-mix.js` | dashboard sessions at `SESS_RATE`/min, per-session polling (devices 5s, agents 5s, integrations 15s, wakes 10s), optional wake | realistic browser load; rate-limit calibration | zero 429 with limits on; per-endpoint p95 < 300ms |
| R5 | `r5-wol-e2e.mjs` | real user + real agent container + real RSA-encrypted MAC; `R5_ROUNDS` wake round-trips | core business SLO | p95 < 3000ms |
| R6 | `r6-soak.js` | 2m ramp → hold (`DURATION`) → 2m down; concurrent SSE VUs (`SOAK_CONNS`) + dashboard sessions (`SOAK_SESS_RATE`/min) | leaks (RSS drift), PG bloat, fd growth | RSS drift < 10% after plateau; bloat < 20%; zero 429 |
| R7 | `r7-static.js` | first-load arrivals: HTML + immutable asset + 304 revalidation | origin bandwidth spent on static (all-miss worst case) | informational: KB/visit against the 375KB/s budget |

The S-series files remain runnable (`sse-connections.js`, `auth-cpu.js`, `command-qps.js`, `wol-e2e.js`, `api-mixed.js`): S1 is the bare-rig flagship; S3b and S5 are superseded by R5 and R4 and kept for continuity.

R4 simulates many client IPs by pinning one fake `X-Forwarded-For` per session (the `trust_proxy` extraction path) — otherwise a single k6 container would squeeze every session into one per-IP bucket; this is the same header chain Cloudflare produces in production.

Compression negotiation (measured, binding): Caddy compresses API JSON and static assets only when the client negotiates (`Accept-Encoding`), and never compresses `text/event-stream` — which matches Cloudflare production behavior, so SSE bandwidth numbers are wire-accurate regardless. k6's `http` module sends no `Accept-Encoding` by default (xk6-sse does send `gzip`), so every API/static scenario must set `Accept-Encoding: gzip` explicitly or it measures uncompressed traffic and overstates per-request bandwidth cost (found the hard way in the first R4/R7 runs); only `gzip` is offered because k6 auto-decompresses gzip but not zstd — production browsers can negotiate zstd, making the rig's numbers slightly conservative.

xk6-sse v0.1.12 constraints (measured, binding for every SSE scenario): `sse.open(url, params, handler)` blocks until the connection closes or `params.timeout` elapses — always pass a timeout so iterations finish; k6 metric objects and `check()` are unusable inside the SSE event callbacks (the native callback boundary drops their goja bindings — `Object has no member`), so scenarios set a local flag in the callback and do all measuring after `open()` returns; per-VU state persists in module-level variables across iterations, which is how the agent backoff state is replayed in R3.

## 4. Seed contract (`wakewake-seed`)

Per count N: N users (`seed-%06d@load.wakewake.local`, one shared precomputed bcrypt hash of `TestPass123!` at the server's cost), N agents (real `generate_pairing_code()`, deduped), 2N devices (per-user cap mirrors `MAX_DEVICES_PER_USER`; `mac_encrypted` is a 344-char base64 placeholder — the exact base64 length of an RSA-2048 OAEP ciphertext — so state-snapshot payloads and storm bandwidth keep production shape), and 30% of users carry a bemfa integration (`--intg-fraction`, config `{"uid":"<344-char placeholder>"}`). A post-COPY consistency check asserts FK integrity and cipher shape. Seeding is not idempotent: reruns require a fresh volume (`compose down -v`) or a `DELETE FROM users WHERE email LIKE 'seed-%'`.

## 5. Judgement methodology

The SLO set (in business terms, not hardware percentages): wake end-to-end p95 < 3s; login p99 < 1.5s; SSE connect p95 < 1s; dashboard API p95 < 300ms; error rate < 0.1%. Saturation is *any* of: one resource (CPU / memory / egress / fds) above 70% for 5 consecutive minutes, any SLO breach, or error rate above 0.1%. The **knee** is the highest staircase level where nothing is saturated; the **sweet spot** is knee × 0.7 (30% headroom); the **extremes** are reported per resource dimension (where each of CPU, bandwidth, memory individually hits 100%), never as a single number. Bandwidth evidence comes from the wan-sampler's 5s `tc -s qdisc` snapshots (differenced bytes = egress rate, drops = shaping pressure), not from k6 throughput alone.

## 6. Red lines

The single source of truth for thresholds is the `THRESHOLD_*` constants in `tests/load/analyze-regression.py`: per-connection RSS slope < 4.0KB (theory 1.2–1.5KB + tokio task header + allocator overhead; measured 3.073KB), R² ≥ 0.95, 100k extrapolation within 110–500MB (measured 315MB). The CPU slope is informational (near-flat expected; a steep rise is a knee signal, not an automatic failure). `--check-only` is the CI-facing gate over `results/regression.md`. Changing a threshold means changing the constant here and this spec in the same change.

## 7. CI

`.github/workflows/load.yml` (manual `workflow_dispatch` only — load tests do not run on push): builds the helper images, runs `run-prod-sim.sh` with the chosen scenario on a 4-vCPU runner (seed compiles inside `rust:1.95-slim` via `SEED_DOCKER_IMAGE`), uploads `tests/load/results/` as an artifact, and runs the red-line check. Full-scale capacity work stays on the dev box: `run-full-matrix.sh` sequences seed-once → R7 → R3 → R1 → R4 → R5 → R2 (limits off) → R6 soak, pins the loader away from the SUT cores (`K6_CPUSET`), adds the 30ms±5 netem hop, and appends every run to `results/manifest.csv`; CI covers wiring regressions and small-scale calibration.

## 8. Cloudflare free-tier assumptions baked into the numbers

The 30s SSE heartbeat covers the edge idle timeout (100–120s) with one lost-beat tolerance; agent traffic traverses the proxy (no DNS-only bypass — the origin IP stays hidden); static distribution uses the origin headers from `docker/caddy/routes.caddy` (`/_next/static/*` immutable 1y; HTML shell `max-age=0, must-revalidate`) plus one Cache Rule giving the HTML shell a short edge TTL — so origin static egress converges to the miss rate times the R7 per-visit volume. Whether Cloudflare free connects to the origin over HTTP/1.1 (the expected case — HTTP/2-to-origin is a paid feature) matters only at extreme agent counts: per-connection TCP state replaces h2c multiplexing, which is why memory numbers are read from the prod-sim rig and not assumed from the bare rig.
