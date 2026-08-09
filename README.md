# WakeWake

**English** | [简体中文](./README_zh.md)

A self-hosted **Wake-on-LAN (WoL)** service. Send a magic packet to any device
on your home network from a browser, anywhere — without exposing the LAN to the
internet.

WakeWake is built as a **Rust** backend (axum + sqlx + tokio) with a standalone
**WoL agent** you run inside the LAN, plus a **Next.js 16** static frontend,
shipped as one multi-arch Docker image behind Caddy. It is designed to hold
**100,000 concurrent agents** on a single 2 GB VPS.

> **Status:** early. No tagged release yet — currently in self-hosted
> "dogfooding". APIs and config may change.

---

## How it works

```
            ┌─────────────────────────────┐
            │   Cloudflare (DNS / CDN)    │  optional
            │   HTTPS → origin            │
            └──────────────┬──────────────┘
                           │ 443 / h2
            ┌──────────────▼──────────────┐
            │   Caddy  (TLS + reverse     │   2 GB VPS
            │   proxy + static frontend)  │
            │   ┌─────────┐ ┌───────────┐ │
            │   │ static  │ │ Rust API  │ │   axum, HTTP/2 (h2c)
            │   │ frontend│ │  + SSE Hub│ │   PostgreSQL 17
            │   └─────────┘ └─────┬─────┘ │
            └─────────────────────┼───────┘
                                  │ HTTPS (agent dials out)
            ┌─────────────────────▼───────┐
            │   wakewake-agent (Rust)     │   your home LAN
            │   • SSE client (commands)   │
            │   • RSA-decrypts the MAC    │
            │   • sends UDP magic packet  │
            │   • Bemfa MQTT (optional)   │
            └─────────────┬───────────────┘
                          │ UDP 255.255.255.255:9
                   ┌──────▼──────┐
                   │  NAS / PC   │
                   └─────────────┘
```

Core design principles:

- **The agent is stateless** — only `pairing_code`, `server_url`, and an RSA
  keypair on disk; full state is pushed on connect.
- **The agent only dials out** — no inbound holes from the server.
- **The server never sees the MAC in plaintext** — MACs are RSA-OAEP + SHA-256
  encrypted in the browser; only the agent holds the private key.
- **PostgreSQL is the single source of truth**; the in-memory command channel
  is ephemeral.

See [`specs/architecture/`](./specs/architecture/) for the full topology,
component responsibilities, and data-flow diagrams.

## Features

- **Wake-on-LAN** from any browser, with device grouping and wake history
- **Standalone Rust agent** — SSE client, RSA key management, magic-packet WoL
- **End-to-end MAC encryption** (RSA-OAEP + SHA-256) via Web Crypto in the
  browser — the server is a ciphertext relay and never holds plaintext MACs
- **Bemfa MQTT integration** — bridge devices to the [Bemfa](https://bemfa.com)
  IoT cloud (device-sync-v3 lifecycle with bounded eventual consistency)
- **Auth** — JWT access + rotating refresh tokens, bcrypt, stateless
  HMAC password-reset, Proof-of-Work anti-abuse on email endpoints
- **i18n** — 8 locales (en, zh, ja, ko, de, fr, es, pt) via next-intl
- **Multi-arch Docker** — one image for `linux/amd64` + `linux/arm64`, s6
  process supervisor, Caddy for TLS (ACME) and static hosting
- **Built for scale** — Rust async tasks (~1.2 KB/connection), SSE-first,
  designed for 100k agents on a 2 GB VPS

## Tech stack

| Layer | Technology | Version |
|---|---|---|
| Backend | Rust, axum, sqlx, tokio | edition 2021, MSRV 1.95 |
| Web | axum 0.8 (HTTP/2 h2c), hyper 1, tower-http 0.6 | |
| Database | PostgreSQL | 17 |
| Auth | jsonwebtoken 10, bcrypt 0.19 (cost 10), RSA-OAEP+SHA-256 | |
| Agent | Rust, reqwest 0.12 (SSE), rumqttc 0.24 (Bemfa MQTT) | |
| Frontend | Next.js 16 (`output: "export"`), React 19, TypeScript | |
| Styling / UI | Tailwind CSS 4, @base-ui/react, Zustand, TanStack Query | |
| Reverse proxy | Caddy 2 (TLS / ACME / h2c / static files) | |
| Container | Docker + docker buildx, s6-overlay (multi-process image) | |

## Quick start (local dev)

Requirements: Docker, Python 3, Node.js 22 + pnpm 11, Rust 1.95.

```bash
git clone https://github.com/jukanntenn/wakewake.git
cd wakewake

# Start the full stack (Postgres + backend + Caddy + frontend dev server)
python3 devops/dev.py start

# Stop everything
python3 devops/dev.py stop
```

Frontend dev server: `http://localhost:${FRONTEND_PORT:-3034}`.

### Manual / per-component

```bash
# Backend (from repo root)
cd backend
cargo test --workspace          # all tests (needs a live PostgreSQL)
cargo test --lib                # unit tests only (no DB)
cargo build --release
cargo clippy --all-targets --all-features -- -D warnings

# Frontend
cd frontend
pnpm install
pnpm dev                        # dev server
pnpm build                      # static export → out/
pnpm test:run                   # Vitest
pnpm lint && pnpm format:check

# E2E (spins up the full container stack)
cd e2e && pnpm test
```

> **Note on dev tooling:** `devops/dev.py` orchestrates a Dockerized backend
> for convenience. If you prefer, run the backend directly with `cargo run`
> and the frontend with `pnpm dev`. See [`AGENTS.md`](./AGENTS.md) for the
> canonical command reference.

## Deployment

WakeWake ships as a single all-in-one Docker image (frontend + Rust backend +
Caddy, managed by s6-overlay) and a sidecar PostgreSQL. Multi-arch
(`linux/amd64`, `linux/arm64`) builds are produced by
[`docker/build.py`](./docker/build.py).

```bash
# 1. Configure environment
cp docker/.env.example docker/.env
#   → edit JWT_SIGNING_KEY, REFRESH_SIGNING_KEY, PASSWORD_RESET__SECRET
#     (openssl rand -base64 32) and POSTGRES_PASSWORD

# 2. Bring up the stack
docker compose -f docker/docker-compose.yml up -d

# 3. Open the app
#   https://<your-host>:8443
```

Migrations run automatically on server startup (`sqlx::migrate!`); there is no
separate migration step. A bootstrap admin is created idempotently on first
start (see `.env.example` → `WAKEWAKE_SECURITY__BOOTSTRAP_*`).

### The agent

The agent (`wakewake-agent`) runs on a machine inside your LAN. Build and run
it locally, or use the published image — see
[`backend/crates/agent/README.md`](./backend/crates/agent/README.md) for the
full guide (config file, env vars, CLI args, self-signed-TLS mode).

```bash
cd backend
cargo build --release -p wakewake-agent
./target/release/wakewake-agent \
  --server https://your-wakewake.example.com \
  --pairing-code <code from the agents page>
```

## Project structure

```
backend/
  crates/
    protocol/   shared wire types (server + agent)
    server/     axum HTTP server (the backend binary)
    agent/      standalone WoL agent (SSE + RSA + Bemfa MQTT)
    seed/       load-test seed binary
  migrations/   sqlx migrations (embedded, applied on startup)
frontend/       Next.js 16 app (static export)
e2e/            Playwright (chromium; separate workspace)
docker/         multi-arch Dockerfiles + build.py + compose
devops/         dev.py + docker-compose + Caddyfile + ansible/
specs/          design specs (architecture, api, database, testing)
.github/        CI workflows (lint / test / build / nightly e2e)
```

## Testing

| Layer | Tooling |
|---|---|
| Backend unit | `cargo test --lib` |
| Backend integration | `cargo test --workspace` (real PostgreSQL — never mocked) |
| Frontend unit | Vitest + Testing Library + jsdom |
| E2E | Playwright (chromium), nightly + on push to `main` |

Test pyramid: ~70% unit / ~25% integration / ~5% e2e. The database, SSE Hub,
and internal logic are never mocked; external HTTP (Bemfa, mailer) is. See
[`specs/testing/`](./specs/testing/) for the strategy.

## License

[GNU AGPL-3.0](./LICENSE). © wakewake contributors.
