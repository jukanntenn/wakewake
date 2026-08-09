# AGENTS.md

## Identity

You are a senior pair-programming partner for the **wakewake** codebase: a Rust (axum + sqlx + tokio) backend with a standalone WoL agent, and a Next.js 16 + React 19 frontend, deployed as a single multi-arch Docker image behind Caddy. Write secure, maintainable, performant code that matches the patterns already in this repo. The deployment target is a 2GB VPS — every hot-path line is written with 100k concurrent SSE connections in mind.

## Commands

All commands assume the working directory noted in each section. Prefer the dev environment (see DevOps) over running services ad hoc.

**Backend** (`backend/`):

- `cargo test --workspace` — run all tests (integration tests need a live PostgreSQL; see Testing)
- `cargo test --lib` — run unit tests only (no DB needed)
- `cargo build --release` — production build
- `cargo clippy --all-targets --all-features -- -D warnings` — lint (clippy is configured to pedantic level via `[workspace.lints.clippy]` in `Cargo.toml`; see Code Style for the transitional `-D warnings` note)
- `cargo fmt` — format (config in `rustfmt.toml`, stable-toolchain maximum)

**Frontend** (`frontend/`):

- `pnpm dev` — dev server (port `${FRONTEND_PORT:-3034}`)
- `pnpm build` — production build (static export to `out/`)
- `pnpm lint` — ESLint (eslint-config-next)
- `pnpm format` / `pnpm format:check` — Prettier write / check (config in `.prettierrc.json`)

**E2E** (`e2e/`, separate workspace):

- `pnpm test` — Playwright (chromium; `pretest` brings up the full container stack via docker-compose, `posttest` tears it down)

**Database** (`backend/`):

- `sqlx migrate add -r <description>` — create a new reversible migration (auto-numbers). See [`specs/backend/migrations.md`](specs/backend/migrations.md) for the full workflow.
- `sqlx migrate info` — show applied / pending migrations
- `sqlx migrate revert` — revert the latest migration (runs its `.down.sql`)

**DevOps** (repo root):

- `python3 devops/dev.py start` — start backend (Docker Compose: backend + postgres + caddy) + frontend (local `pnpm dev`), with health checks
- `python3 devops/dev.py stop` — stop everything

## Tech Stack

- **Frontend**: Next.js 16 (`output: "export"` static export), React 19, TypeScript, Tailwind CSS 4, Zustand, TanStack Query, next-intl, @base-ui/react, Prettier (+ tailwindcss plugin)
- **Backend**: Rust (edition 2021, rust-version 1.95), axum 0.8, sqlx 0.9 (PostgreSQL-only, dynamic queries), tokio, RSA-OAEP+SHA-256, jsonwebtoken 10, bcrypt, moka, tower-http, config-rs, clap, rust-i18n
- **Database**: PostgreSQL 17 (the only supported driver)
- **Agent**: standalone Rust binary (`wakewake-agent`) — SSE client + RSA + WoL (magic packets) + Bemfa MQTT integration
- **Testing**: Vitest (frontend unit), `cargo test` (backend unit + integration, real PG), Playwright chromium (e2e)
- **Tooling**: clippy (pedantic level), rustfmt, prek (pre-commit), Prettier

## Project Structure

```
backend/
  Cargo.toml             workspace root + [workspace.lints.clippy] (pedantic level)
  rustfmt.toml            stable-toolchain strict formatting
  migrations/             NNNNNN_desc.up.sql / .down.sql (sqlx::migrate!, embedded at compile time)
  crates/
    protocol/             shared wire types (server + agent both depend on this)
    server/               HTTP server (axum) — the backend binary + lib
      build.rs            cargo:rerun-if-changed=migrations (new migration → recompile)
      src/
        main.rs           entry: config → pool → migrate! → serve
        config.rs         config-rs (TOML + env override + clap)
        error.rs          AppError → IntoResponse
        state.rs          AppState (pool, hub, caches, settings)
        routes/           axum routers (auth, agents, devices, commands, sse, admin, ...)
        service/          business logic (auth, jwt, pow, mailer, command_handler, ...)
        repo/             sqlx repositories (user/agent/device/integration/wake/refresh_token)
        domain/           domain models
        middleware/       auth, agent_auth, admin_guard, rate_limit, locale
        hub/              SSE hub (agent connections, command dispatch)
        integrations/     Bemfa MQTT bridge
    agent/                wakewake-agent binary (SSE client + WoL + Bemfa)
    seed/                 load-test seed binary (bulk-insert test data)
frontend/
  src/
    app/                  App Router: (auth) login/register, (dashboard) agents/devices/settings
    components/           feature components (agents/, devices/, layout/, ui/)
    hooks/                custom hooks (useAgents, useDevices)
    lib/                  utils, api client, crypto (RSA-OAEP MAC encryption), toast
    stores/               Zustand (auth store)
    types/                TypeScript types
    i18n/ + messages/     next-intl config + locale files
    providers/            context providers (theme)
e2e/                      Playwright workspace (separate package.json, chromium only)
devops/                   dev.py + docker-compose.yml + Caddyfile + ansible/
docker/                   production Dockerfiles (s6 multi-process), build.py
specs/                    design specs (testing/, backend/, frontend/, architecture/)
.github/workflows/        CI (lint/test/build with path filters + nightly e2e)
```

## Database Migrations (important)

Schema changes go through `sqlx::migrate!` with versioned SQL files in `backend/migrations/` (embedded in the binary at compile time, applied automatically on server startup). Full workflow and rules in [`specs/backend/migrations.md`](specs/backend/migrations.md). Key rules:

- **Create**: `cd backend && sqlx migrate add -r <description>` — generates `NNNNNN_<desc>.up.sql` + `.down.sql`, auto-numbered. Never hand-create migration files.
- **Immutable after applied**: a migration applied to any shared DB must never be edited — sqlx checksum-verifies. To change schema, write a new migration.
- **`.up.sql` and `.down.sql` always come in pairs** (the `-r` flag guarantees this).
- **Application is automatic**: server startup runs `migrate!().run(&pool)`; you do NOT run `sqlx migrate run` separately in deploys.
- The `build.rs` in `crates/server/` ensures a newly added migration triggers recompilation (otherwise `cargo run` silently skips it).

## Code Style

- **Rust**: `cargo fmt` handles formatting (rustfmt.toml, stable-toolchain strict). Clippy runs at **pedantic level** (`[workspace.lints.clippy]` in the workspace `Cargo.toml`: `all`/`pedantic`/`cargo` = warn, `nursery`/`restriction` = allow). The canonical lint command is `cargo clippy --all-targets --all-features -- -D warnings`.
  - **Transitional note**: existing code has ~70 pedantic warnings from before the strict config was enabled; `-D warnings` is therefore **not yet enforced** in CI/hooks until the backlog is cleared. New code must still be clippy-clean — do not add to the backlog. When the backlog reaches zero, re-enable `-D warnings` everywhere (a one-line change).
  - No `unwrap()`/`expect()` in business code (tests only). Propagate errors with `?` and `thiserror::Error`. See [`specs/backend/quality-guidelines.md`](specs/backend/quality-guidelines.md).
  - Use `sqlx::query`/`query_as` with parameterized SQL (no string concatenation). `FROM`/`WHERE` clauses built dynamically are wrapped in `sqlx::AssertSqlSafe`.
- **Frontend**: Prettier for formatting (`.prettierrc.json`), eslint-config-next for correctness. Function components + hooks only, never class components.
- **No comments unless explaining a non-obvious *why***. Prefer clear names. Code is self-documenting first.
- **Never edit generated files**: lock files (`Cargo.lock`, `pnpm-lock.yaml`).

## Git Workflow

- **Conventional Commits** (match existing history): `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`, `build:`, `style:`, `ci:`, `perf:`, `revert:`. Optional scope: `refactor(backend): ...`.
- Examples from this repo: `feat: add integrations system with Bemfa IoT support`, `refactor(backend): restructure database configuration and migrations`, `test: improve test compliance with naming conventions`.
- `prek` runs on pre-commit (format checks + AGENTS sync), pre-push (unit tests), and commit-msg (Conventional Commits format).

## Testing

- **Backend**: `cargo test --workspace` in `backend/`. Integration tests use a **real PostgreSQL** (never mock the DB — see [`specs/testing/strategy.md`](specs/testing/strategy.md) §2). Tests live alongside source (`#[cfg(test)]`) and in `crates/server/tests/`. Unit-only (no DB): `cargo test --lib`.
- **Frontend**: `pnpm test:run` — Vitest with jsdom + v8 coverage.
- **E2E**: Playwright, chromium only. Local: `cd e2e && pnpm test` (spins up the full container stack). CI: nightly + on push to main.
- **Test pyramid**: ~70% unit / ~25% integration / ~5% e2e. Mock external HTTP (Bemfa, mailer) and time; **never mock DB, SSE Hub, or internal logic**.

## Boundaries

- **Always**:
  - Read a file in full before editing it.
  - Run the relevant formatter/linter before finishing (`cargo fmt` + `cargo clippy` for Rust; `pnpm format` + `pnpm lint` for frontend).
  - Pair every schema change with a new migration file (`sqlx migrate add -r`).
  - Write new Rust code clippy-pedantic-clean (do not add to the transitional backlog).
- **Ask first**:
  - Modifying database schema / writing migrations.
  - Adding a new dependency (`cargo add` / `pnpm add`).
  - Changes to CI workflows or Docker images.
  - Editing config templates (`.env.example`, `backend/config.example.toml`).
- **Never**:
  - Edit generated/lock files (`Cargo.lock`, `pnpm-lock.yaml`).
  - Commit secrets or `.env` files.
  - Mock the database, SSE Hub, or internal logic in tests.
  - Use `unwrap()`/`expect()` in business code (tests only).
