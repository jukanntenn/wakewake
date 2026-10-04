# Configuration spec (server + agent + deployment surfaces)

English | [中文](configuration.zh.md)

> This document is the single specification for wakewake configuration: loading layers, the server/agent schemas, environment variable mapping, example-file conventions, and the full picture of "where configuration lives, where secrets live" per deployment environment. Operational procedures (build/release/promotion gates) live in [`../../devops/README.md`](../../devops/README.md); this document owns mechanisms and invariants only.

## 1. Design principles

1. **TOML files first, environment variables override, CLI arguments win.** Deployment-level configuration goes into `config.toml` (configured once, diffable, reviewable); `WAKEWAKE_*` environment variables serve container injection and dynamic test overrides; CLI arguments serve one-off debugging overrides.
2. **Server and agent share one architecture, split files**: two independent `Settings` (different fields) under one set of conventions (config-rs + clap + three-layer override + `WAKEWAKE_` prefix + `__` nesting separator).
3. **Fail fast**: a failed configuration validation (garde) → startup failure, never degraded running.
4. **The example files are the primary user documentation** (see §7); the schema source of truth is the `Settings` struct + builder `set_default` (server: `crates/server/src/config.rs`; agent: `crates/agent/src/config.rs`).

Technology choice: config-rs 0.15 (`default-features = false, features = ["toml"]`, dropping the yaml/json/ron parsers) + clap 4 derive (CLI parsing and `--help`; config-rs does not replace clap — orthogonal responsibilities).

## 2. The three-layer override model (highest priority first)

```
CLI 参数（clap → set_override_option）  >  环境变量（WAKEWAKE_*）  >  TOML 文件  >  内置默认值
```

| Layer | Implementation | Purpose |
|---|---|---|
| Built-in defaults | `Config::builder().set_default(key, value)` | Safe defaults (ports, TTLs — non-sensitive items) |
| TOML file | `File::with_name(path).required(false/true)` | Deployment-level configuration (the main carrier) |
| Environment | `Environment::with_prefix("WAKEWAKE").prefix_separator("_").separator("__").try_parsing(true)` | Container injection (DSN assembly), e2e/test dynamic overrides |
| CLI arguments | `set_override_option(key, clap_value)` | Debug / one-off overrides |

Loading-order details (server `config.rs::Settings::load`):

- **File search**: with `--config <path>` the file must exist (`required(true)`); without it, the working directory's `./config.toml` is searched (`File::with_name("config").required(false)` — no error if absent).
- **`__` (not `_`) as the nesting separator**: a single `_` collides with underscores inside TOML keys (e.g. `packet_delay_ms` would be mis-split into nesting). `WAKEWAKE_SERVER__HOST` → `server.host`.
- **`try_parsing(true)`**: env strings are tried as `i64`/`f64`/`bool` (`"8080"` → `i64` → `u16`; stays a string when parsing fails); bools accept `true/false/1/0`.
- **`set_override_option`**: an unset clap value (`None`) does not override; a set one is the highest priority.
- **The agent's top-level keys use a single `_`**: `WAKEWAKE_SERVER_URL` / `WAKEWAKE_PAIRING_CODE` / `WAKEWAKE_HOME` (no nesting at the top level, outside the `__` scheme).

## 3. Server configuration schema

### `[app]` (required)

| Field | Type | env | Notes |
|---|---|---|---|
| `public_url` | string | `WAKEWAKE_APP__PUBLIC_URL` | Public origin (scheme+host[+port]). Email links / agent doc URLs are built from it. **Never derived from the request Host** (forgeable behind a reverse proxy). Startup fails when missing |

### `[server]`

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `host` | string | `0.0.0.0` | `WAKEWAKE_SERVER__HOST` | Listen address |
| `port` | u16 | `8080` | `WAKEWAKE_SERVER__PORT` | **Invariant 8080**: the in-image `docker/caddy/routes.caddy` proxies to `127.0.0.1:8080`; never changes |
| `trust_proxy` | bool | `true` | `WAKEWAKE_SERVER__TRUST_PROXY` | true = client IP from the leftmost XFF (reverse proxy); false = TCP peer (direct deployment) |
| `client_ip_header` | Option\<string\> | unset | `WAKEWAKE_SERVER__CLIENT_IP_HEADER` | Authoritative client-IP header (e.g. `CF-Connecting-IP` behind Cloudflare), read before XFF/X-Real-IP when set and `trust_proxy` is true. Behind CF the XFF chain is append-mode (leftmost forgeable), so prod sets this; it requires the access layer to admit only trusted proxies (the host gateway site block's `remote_ip` CF-CIDR guard) or the header itself is forgeable — the two ship together |
| `max_sse_connections` | usize | `2000` | `WAKEWAKE_SERVER__MAX_SSE_CONNECTIONS` | Global SSE connection cap (0 = unlimited); connections beyond it get 503 SERVICE_UNAVAILABLE and agents reconnect with backoff. Abuse backstop, not a per-user quota (connection cardinality = paired agents; same-agent reconnects replace, never stack) |

### `[database]` (required)

| Field | Type | env | Notes |
|---|---|---|---|
| `dsn` | string | `WAKEWAKE_DATABASE__DSN` | PG connection string. Container topologies standardize on the Unix socket: `postgres:///db?host=/var/run/postgresql&user=...&password=...&sslmode=disable` |

DSN single-source rule (the password is written once):

- Self-deploy / local acceptance: compose interpolates from `.env`'s `POSTGRES_*` and injects the env;
- ansible environments: `config.toml.j2` renders into TOML, the password passed through `urlencode` (vault values may contain URL-special characters);
- e2e: compose ships dedicated test credentials.

### `[jwt]`

| Field | Type | Default | env | Validation | Notes |
|---|---|---|---|---|---|
| `signing_key` | string | — | `WAKEWAKE_JWT__SIGNING_KEY` | `length(min=32)` | access HMAC key |
| `refresh_signing_key` | string | — | `WAKEWAKE_JWT__REFRESH_SIGNING_KEY` | `length(min=32)` | refresh HMAC key (separate; never reuse) |
| `access_expire` | humantime | `15m` | `WAKEWAKE_JWT__ACCESS_EXPIRE` | — | e2e short-ttl overrides with `10s` |
| `refresh_expire` | humantime | `720h` | `WAKEWAKE_JWT__REFRESH_EXPIRE` | — | 30d |

Keys have no default; generate with `openssl rand -base64 32`.

### `[password_reset]`

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `secret` | string | — | `WAKEWAKE_PASSWORD_RESET__SECRET` | Stateless reset-token HMAC key (required) |
| `expire` | humantime | `1h` | `WAKEWAKE_PASSWORD_RESET__EXPIRE` | token TTL |

### `[pow]`

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `difficulty` | u8 | `4` | `WAKEWAKE_POW__DIFFICULTY` | Startup default for the leading-zero count; runtime-adjustable via `POST /admin/pow` (0..=10), persisted to `data/pow.json` |
| `challenge_ttl` | humantime | `10m` | `WAKEWAKE_POW__CHALLENGE_TTL` | Challenge validity window |

### `[mailer]`

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `enabled` | bool | `false` | `WAKEWAKE_MAILER__ENABLED` | When false, password-reset endpoints return 503 |
| `smtp_host` | Option | — | `WAKEWAKE_MAILER__SMTP_HOST` | SMTP server |
| `smtp_port` | u16 | `587` | `WAKEWAKE_MAILER__SMTP_PORT` | SMTP port |
| `smtp_username` | Option | — | `WAKEWAKE_MAILER__SMTP_USERNAME` | Username |
| `smtp_password` | Option | — | `WAKEWAKE_MAILER__SMTP_PASSWORD` | Password |
| `from_address` | Option | — | `WAKEWAKE_MAILER__FROM_ADDRESS` | Sender address |
| `from_name` | string | `WakeWake` | `WAKEWAKE_MAILER__FROM_NAME` | Sender name |
| `max_register_emails_per_day` | u32 | `500` | `WAKEWAKE_MAILER__MAX_REGISTER_EMAILS_PER_DAY` | Per-path UTC-daily budget (0 = unlimited); runtime-adjustable via `POST /admin/mailer`, counters persisted to `data/mailer.json` |
| `max_resend_emails_per_day` | u32 | `200` | `WAKEWAKE_MAILER__MAX_RESEND_EMAILS_PER_DAY` | Same mechanism as above, resend path |
| `max_reset_emails_per_day` | u32 | `300` | `WAKEWAKE_MAILER__MAX_RESET_EMAILS_PER_DAY` | Same mechanism as above, reset path |

### `[rate_limit]` (test-only)

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `disabled` | bool | `false` | `WAKEWAKE_RATE_LIMIT__DISABLED` | true = bypass the axum-governor rate limiter. **Bypasses rate limiting only, never business quotas** (`MAX_DEVICES_PER_USER` and friends are hardcoded consts). Always false in production; true for e2e/load tests |

### `[security]` (bootstrap admin)

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `bootstrap_admin_email` | string | `admin@wakewake.local` | `WAKEWAKE_SECURITY__BOOTSTRAP_ADMIN_EMAIL` | Created idempotently at startup (created only if absent, never overwritten). Grafana's default admin pattern |
| `bootstrap_admin_password` | string | `wakewake123` | `WAKEWAKE_SECURITY__BOOTSTRAP_ADMIN_PASSWORD` | ⚠️ production must override the default |
| `unverified_retention_days` | u32 | `7` | `WAKEWAKE_SECURITY__UNVERIFIED_RETENTION_DAYS` | Daily purge deletes unverified non-superuser accounts older than this (0 = off); see [risk controls](risk-controls.md) |

### `[log]`

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `level` | string | `info` | `WAKEWAKE_LOG__LEVEL` | **Priority: `RUST_LOG` (per-module, e.g. `wakewake_server=debug,sqlx=warn`) > this field**. serve and the admin subcommands share this fallback chain |
| `dir` | string | `data/logs` | `WAKEWAKE_LOG__DIR` | tracing-appender daily-rolling directory (4 files: app/traces/metrics/logs). Inside the container `./data:/app/data` is already mounted, so it persists |

### `[maintenance]` (ops mitigation, initial value)

| Field | Type | Default | env | Notes |
|---|---|---|---|---|
| `enabled` | bool | `false` | `WAKEWAKE_MAINTENANCE__ENABLED` | The config file decides only the **initial value**; toggled at runtime via `POST /admin/maintenance` and persisted to `data/maintenance.json` (restored on restart) |
| `mode` | enum | `registration_disabled` | `WAKEWAKE_MAINTENANCE__MODE` | `registration_disabled` / `readonly` / `full` (admin always passes) |
| `message` | string | fixed English copy | `WAKEWAKE_MAINTENANCE__MESSAGE` | Interception response copy |

### Environment variables without the `WAKEWAKE_` prefix

| Variable | Consumer | Notes |
|---|---|---|
| `RUST_LOG` | tracing EnvFilter | Highest priority for service log level (see `[log].level`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | observability | Set → OTLP collector; unset → local file exporter |
| `TZ` | s6 `cont-init.d/01-setup.sh` | Container timezone |

### CLI (`wakewake-server`)

`--config <path>`; subcommands: `serve [--host] [--port]` (default behavior), `version`, `admin create|promote|demote|list` (admin subcommands share the same `log.level` fallback chain).

> The server's `Settings` carries **no `[wol]`** — the server never sends magic packets.

## 4. Agent configuration schema

The agent shares the server's framework with different fields. File search path (in order; later overrides earlier): `$WAKEWAKE_HOME/config.toml` (default `$HOME/.wakewake/`) → `./wakewake.toml` → `--config <path>` explicit (must exist; skips the first two).

```toml
# agent 的 config.toml
server_url   = "https://wakewake.app"     # 必填；内网自签需编译 --features danger-insecure-tls
pairing_code = "a1b2c3d4e5f60718"         # 必填，从 server agents 页获取

[wol]
broadcast_addr  = "255.255.255.255:9"
packet_count    = 3
packet_delay_ms = 50

[log]
level  = "info"      # 优先级 RUST_LOG > 本项
format = "pretty"    # pretty（终端）/ json（容器）；agent 无 dir 键，默认 <home_dir>/logs

[bemfa]
broker   = "bemfa.com"
port     = 9503
# api_base = "http://bemfa-mock:8080"   # 覆盖缝隙，日常不设
```

| Group | Fields | env | Notes |
|---|---|---|---|
| Top level (required) | `server_url` / `pairing_code` | `WAKEWAKE_SERVER_URL` / `WAKEWAKE_PAIRING_CODE` | CLI `--server` / `--pairing-code` wins |
| Top level | `home_dir` | `WAKEWAKE_HOME` | Holds config.toml + key.pem + logs. Priority: env > `$HOME/.wakewake` > `./.wakewake`; `/data` in containers (volume `agent_data`); ansible supervisor points at `app_path` |
| `[wol]` | `broadcast_addr` / `packet_count` / `packet_delay_ms` | `WAKEWAKE_WOL__*` | Magic-packet send parameters |
| `[log]` | `level` / `format` / `dir`(Option) | `WAKEWAKE_LOG__*` | `dir` unset = `<home_dir>/logs` |
| `[bemfa]` | `broker` / `port` / `api_base`(Option) | `WAKEWAKE_BEMFA__BROKER` / `__PORT` / `__API_BASE` | Bemfa endpoints |

`[bemfa].api_base` semantics (important): the real Bemfa cloud scatters its APIs across several domains (`pro.bemfa.com` / `apis.bemfa.com`), so no single value can be the default — **unset = per-endpoint real-domain defaults; set = whole-base-URL override + path reassembly** (`{api_base}/v1/createTopic` etc.). The key's only use is pointing E2E/tests at a mock (`docker-compose.e2e.yml` points at bemfa-mock). With `port == 9503` MQTT uses TLS (system default CAs); any other port (e.g. mosquitto 1883) uses plaintext.

## 5. Validation and fail-fast

After loading, garde validates semantic constraints (config-rs does no validation):

- `app.public_url`: `#[garde(url)]`; both JWT keys `length(min = 32)`; the whole `Settings` gets `dive`.
- A validation failure → process exits 1, never degraded running.

| Failure | Behavior |
|---|---|
| `--config` file does not exist | Startup fails, error includes the path |
| TOML syntax error | Startup fails, error carries the line number |
| Required field missing (no file, no env) | Startup fails, `missing field` |
| Key too short | Startup fails, `length min = 32` |
| Wrong env value type (e.g. non-numeric port) | Startup fails, `invalid type` |

Time fields (`access_expire` etc.) are humantime strings, parsed by `humantime::parse_duration` in the `Settings` accessors (`15m` / `720h` / `10s`).

## 6. Environment variable mapping rules

| Rule | Notes | Example |
|---|---|---|
| Prefix | `WAKEWAKE_` | — |
| Nesting separator | `__` | `WAKEWAKE_SERVER__HOST` → `server.host` |
| Agent top-level keys | single `_` (no nesting) | `WAKEWAKE_SERVER_URL` → `server_url` |
| Case | env all-caps → TOML all-lower | `WAKEWAKE_DATABASE__DSN` → `database.dsn` |
| Type inference | `try_parsing(true)` tries i64/f64/bool | `WAKEWAKE_SERVER__PORT=8080` → u16 |
| Special characters | an env value is one string, no escaping | `WAKEWAKE_DATABASE__DSN="postgres://u:p@db:5432/d"` |

## 7. `config.example.toml` conventions (user-facing documentation)

The example files are the **primary user documentation**; every schema change must sync them. Three exist today, each for a different reader:

| File | Reader | Content |
|---|---|---|
| `backend/config.example.toml` | full schema documentation | every field + notes |
| `backend/crates/agent/config.example.toml` | agent users | every agent field |
| `docker/config.example.toml` | self-deployers | only what they care about (public_url + 3 keys + optional mailer) |
| `docker/config.local.example.toml` | local acceptance | every field listed explicitly (defaults/placeholders), `cp` then edit keys |

Field conventions:

| Field kind | Convention |
|---|---|
| Required, no default (e.g. `jwt.signing_key`) | uncommented, placeholder value `"CHANGE_ME..."`, tagged `[REQUIRED]` |
| Optional with default (e.g. `server.port`) | commented out, value = the default, tagged `[OPTIONAL]` |
| Security-sensitive default | tagged with a `⚠️` warning |

Every field must carry: a one-line description, the `[REQUIRED]`/`[OPTIONAL]` tag, an `Env:` tag, and a `Default:` tag (optional fields only).

## 8. Deployment-surface configuration map (what each environment reads)

Principle: **each environment = one command + at most one file to edit**. The ops cheat sheet lives in [`../../devops/README.md`](../../devops/README.md) (environment quick reference).

| Environment | Entry command | Configuration carrier | Where secrets live |
|---|---|---|---|
| Local dev | `python3 devops/dev.py start` | dev.py generates `backend/config.local.toml` on first start (dev defaults embedded) | fixed dev values (never committed) |
| Local acceptance | `docker compose -f docker/docker-compose.local.yml up` | `docker/config.local.toml` (template `config.local.example.toml`) + compose env (DSN/public_url interpolation) | placeholder keys (never committed) |
| E2E | `cd e2e && pnpm test` | `e2e/docker-compose.e2e.yml` env-only (the override layer used as the primary) + `short-ttl.yml` override | compose-embedded test keys (never shared with production) |
| Self-deploy | `cd docker && docker compose up -d` | `docker/config.toml` (template `config.example.toml`) + `.env` (`POSTGRES_*`) | user-local files (never committed) |
| Remote test/staging/prod | `ansible-playbook devops/ansible/deploy.yml -l <env>` | `group_vars/<env>/env.yml` + `vault.yml` (avpm single-variable encryption) → renders `config.toml.j2` + `docker-compose.yml.j2` + Caddyfile | ansible vault (committed but encrypted) |
| agent (bare-metal) | `deploy-agent.yml -l test_agent` | vault's `agent_server_url`/`agent_pairing_code` → renders `agent-config.toml.j2` + supervisord conf | ansible vault |

ansible environment variables (`group_vars/<env>/env.yml`): `image` (test = LAN registry floating `main`; staging/prod = `wakewake_version` pinned), `host_port` (+ `loopback_publish` = loopback-only publish, prod), `public_url`, `health_url`/`health_insecure`, `tls_profile` (http|https; branches the healthcheck and the caddy-data volume), `caddyfile_template` (undefined = zero mounts, the image-builtin Caddyfile), the prod host-gateway vars (`gateway_domain`/`gateway_site_file`/`gateway_certs_dir`/`cloudflare_cidrs`), `postgres_gucs`, `mailer_*`. Shared invariants live in `group_vars/all.yml` (8080/8443, PG database/user, paths, default `postgres_gucs`).

### Caddyfile: 3 site variants + 1 routing source of truth + the prod host gateway

The single routing source of truth is `docker/caddy/routes.caddy` (baked into the image at `/app/caddy/routes.caddy`): API/SSE proxying + static frontend + security headers + access logs. **To change routing, change this one file.** Each environment's Caddyfile does exactly two things: import the routes and declare the site address and TLS layer:

| Variant | TLS shape | Environments |
|---|---|---|
| `docker/Caddyfile` (image-builtin `/app/Caddyfile`) | hostless `:8443` plain HTTP | self-deploy + staging + prod (zero mounts) |
| `docker/Caddyfile.local` | `tls internal` self-signed (SAN localhost, app) | local acceptance + e2e (shared) |
| `devops/ansible/templates/Caddyfile.test` | `tls internal` + `fallback_sni` (direct IP) | test |
| `devops/ansible/templates/wakewake.caddy.j2` | CF Origin Cert + CF CIDR guard — a **host** systemd Caddy site block, not a container Caddyfile; reverse-proxies to the loopback-only `host_port` | prod (host gateway on `:443`) |

## 9. Invariants list (hold across every environment)

Changing any of these ripples widely — grep every reference before touching them:

1. **8080**: backend listen port = the `routes.caddy` proxy target (`[server].port` never changes).
2. **8443**: in-container Caddy listen port (the container side of compose `ports`, healthchecks, Caddyfile site addresses).
3. **`/var/run/postgresql`**: the postgres-socket shared-volume path = DSN `host=` = postgres healthcheck `-h` (the postgres image default; do not change).
4. **`/app/Caddyfile`**: the fixed path the s6 caddy run script reads (mount points must agree).
5. **`/app/config.toml`**: the server's default file-search path (working directory `/app`).
6. **`./data:/app/data`**: the persistence root for log rotation + `maintenance.json`.
7. **Service names `app` / `mailpit` / `postgres`**: docker DNS referenced elsewhere (Caddyfile SAN, `smtp_host`, depends_on); renaming must sync all of them.
8. **compose host port and `public_url` share one source**: in local acceptance `APP_PORT` interpolates both `ports` and `WAKEWAKE_APP__PUBLIC_URL`; in remote environments `host_port` ↔ `public_url` appear as a pair in env.yml.

## 10. Secret management

- **Generation**: `openssl rand -base64 32`; the three HMAC keys (jwt access / jwt refresh / password_reset) are mutually independent — never reuse.
- **Dev/e2e**: fixed test values (e2e never shares with production).
- **Self-deploy**: local `config.toml` + `.env`, neither committed (`*.local.toml`, `.env` are ignored).
- **Remote environments**: avpm vault single-variable encryption committed to the repo (rotation in devops/README §Vault).
- **Rotation**: change the value and restart. Rotating JWT keys invalidates all existing tokens (expected; forces re-login).
- **bootstrap admin**: the default `admin@wakewake.local` / `wakewake123` only idempotently bootstraps; production must override the default password via env/TOML.
