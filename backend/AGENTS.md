# backend/AGENTS.md

Backend workspace: Rust (edition 2021, rust-version 1.95), axum 0.8, sqlx 0.9 (PostgreSQL-only), tokio. Crates: `protocol` (shared wire types), `server` (HTTP server + SSE hub), `agent` (standalone WoL binary), `seed` (load-test).

## Commands (`backend/`)

- `cargo test --workspace` — all tests (integration tests need a live PostgreSQL; `python3 ../devops/dev.py init` brings one up)
- `cargo test --lib` — unit tests only (no DB needed)
- `cargo build --release` — production build
- `cargo clippy --all-targets --all-features -- -D warnings` — the canonical lint command; see the transitional note below
- `cargo fmt` — format (config in `rustfmt.toml`, stable-toolchain maximum); the prek `fmt` hook runs it on commit

## Migrations

Schema changes go through `sqlx::migrate!` with versioned SQL files in `migrations/` (embedded in the binary at compile time, applied automatically on server startup — you never run `sqlx migrate run` in deploys). The `build.rs` in `crates/server/` ensures a newly added migration triggers recompilation.

- **Create**: `sqlx migrate add -r <description>` — generates `NNNNNN_<desc>.up.sql` + `.down.sql`, auto-numbered. Never hand-create migration files.
- **Immutable after applied**: a migration applied to any shared DB must never be edited — sqlx checksum-verifies. To change schema, write a new migration.
- **`.up.sql` and `.down.sql` always come in pairs** (the `-r` flag guarantees this) and commit together with the consuming code.

## Code style

- Clippy runs at **pedantic level** (`[workspace.lints.clippy]` in `Cargo.toml`). **Transitional note**: existing code has a pedantic-warning backlog, so `-D warnings` is not yet enforced until the backlog clears — new code must still be clippy-clean; do not add to the backlog.
- No `unwrap()`/`expect()` in business code (tests only). Propagate errors with `?` and `thiserror::Error`.
- `sqlx::query`/`query_as` with parameterized SQL only (no string concatenation); dynamically built `FROM`/`WHERE` clauses are wrapped in `sqlx::AssertSqlSafe`.

## Testing

- Integration tests use a **real PostgreSQL** — never mock the DB. Tests live alongside source (`#[cfg(test)]`) and in `crates/server/tests/`.
- Test pyramid: ~70% unit / ~25% integration / ~5% e2e. Mock external HTTP (Bemfa, mailer) and time; **never mock DB, SSE Hub, or internal logic**.

## Boundaries

- **Ask first**: schema changes / writing migrations; new dependencies (`cargo add`).
- **Never**: edit `Cargo.lock` by hand; block the tokio runtime in handlers; introduce blocking I/O on the async path.
