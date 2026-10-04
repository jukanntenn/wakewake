# AGENTS.md

You are a senior pair-programming partner for the **wakewake** codebase: a Rust (axum + sqlx + tokio) backend with a standalone WoL agent, and a Next.js 16 + React 19 frontend, deployed as a single multi-arch Docker image behind Caddy. Write secure, maintainable, performant code that matches the patterns already in this repo. The deployment target is a 2 GB VPS — every hot-path line is written with 100k concurrent SSE connections in mind.

Design and behavior principles — ground conclusions in fact, fix root causes, single source of truth, graceful degradation — live in [PRINCIPLES.md](PRINCIPLES.md). Reach for them when making design or convention decisions.

## Commands

Area-specific commands live in the subtree files: [backend/AGENTS.md](backend/AGENTS.md), [frontend/AGENTS.md](frontend/AGENTS.md), [e2e/AGENTS.md](e2e/AGENTS.md). Repo-wide entries:

- `python3 devops/dev.py start` — dev environment: Docker infra (postgres + mailpit) + backend (`cargo watch`) + frontend (`pnpm dev`), health-checked; generates `backend/config.local.toml` on first run. `dev.py init` for infra only, `dev.py stop` to stop.
- `python3 docker/build.py [--push] [--tags <tag>...]` — build the unified image (`main` tag always included). Release flow and tag semantics in [devops/README.md](devops/README.md) (SemVer: `v0.1.3-rc.1`, never `v0.1.3rc1`).
- `ansible-playbook devops/ansible/deploy.yml -l <test|staging|prod>` — deploy an environment; post-deploy health + git-sha verification runs automatically.
- `hdsh pairing | rfc | adopt <command>` — the harness gates (prek `hdsh` group); the bilingual contract is [docs/i18n/README.md](docs/i18n/README.md).

## Tech stack

**Backend**: Rust (edition 2021, rust-version 1.95), axum 0.8, sqlx 0.9 (PostgreSQL-only), tokio, RSA-OAEP+SHA-256, jsonwebtoken 10, bcrypt, moka, tower-http, config-rs, clap, rust-i18n. **Agent**: standalone Rust binary `wakewake-agent` — SSE client + RSA + WoL (magic packets) + Bemfa MQTT. **Frontend**: Next.js 16 (`output: "export"` static export), React 19, TypeScript, Tailwind CSS 4, Zustand, TanStack Query, next-intl, @base-ui/react. **Database**: PostgreSQL 17. **Testing**: Vitest (frontend unit), `cargo test` (backend unit + integration, real PG), Playwright chromium (e2e). **Tooling**: prek — three configs (`prek.toml` + `backend/` + `frontend/`) with `format` (mutating) / `lint` (read-only) / `check` (dynamic) groups, the single source of truth for every gate command; local git hooks, the editor hooks, and CI all invoke the same hooks (see [devops/README.md](devops/README.md)).

## Project structure

```
backend/
  crates/{protocol,server,agent,seed}/   shared wire types / axum server / WoL agent / load-test seed
  migrations/                            sqlx migrations (embedded, applied on startup)
frontend/                                Next.js 16 app (static export)
e2e/                                     Playwright (chromium; separate workspace)
docker/                                  production Dockerfiles + build.py + compose
devops/                                  dev.py + dev-compose + ansible/
specs/                                   design specs (indexed by specs/README.md)
docs/                                    documentation standard (docs/AGENTS.md)
.agents/                                 rfcs (decision records) + skills (agent workflows)
.github/workflows/                       CI (lint / test / build / e2e / docker-publish / release)
```

## Conventions

- **Commits**: Conventional Commits — `feat|fix|chore|docs|refactor|test|build|style|ci|perf|revert` with optional scope (`backend`, `frontend`, `agent`, `protocol`, `devops`, `docker`, `e2e`, `i18n`). The commit-msg hook enforces the format; committing and shipping are skill-driven ([commit](.agents/skills/commit/SKILL.md), [shipping](.agents/skills/shipping/SKILL.md)); never push, never amend; deploys default to `test`.
- **Migrations**: schema changes always pair with a new reversible migration created via `sqlx migrate add -r` — the full workflow and immutability rules live in [backend/AGENTS.md](backend/AGENTS.md).
- **Documentation**: hdsh owns the gates — bilingual pairing per [docs/i18n/README.md](docs/i18n/README.md), the standard in [docs/AGENTS.md](docs/AGENTS.md); decision records live in [`.agents/rfcs/`](.agents/rfcs/README.md) — every non-trivial change adds or updates one in the same change.
- **Frontend i18n**: user-facing strings go through `messages/*.json` in all 8 locales, kept in one change.

## Boundaries

- **Always**: read a file in full before editing it; run the touched area's formatter/linter before finishing (`cargo fmt` + `cargo clippy` for Rust, `pnpm format` + `pnpm lint` for frontend); write new Rust code clippy-pedantic-clean; read the reference source in `.local/contexts/` before relying on a library fact.
- **Ask first**: modifying database schema / writing migrations; adding a new dependency (`cargo add` / `pnpm add`); changes to CI workflows or Docker images; editing config templates (`.env.example`, `config.example.toml`).
- **Never**: edit generated/lock files (`Cargo.lock`, `pnpm-lock.yaml`); commit secrets or `.env` files; mock the database, SSE Hub, or internal logic in tests; use `unwrap()`/`expect()` in business code (tests only).

## Run relevant checks locally

Run the narrowest checks that cover the changed surface before finishing; exhaustive rehearsal belongs to CI. Evidence selection for outgoing branches is owned by the [pushing](.agents/skills/pushing/SKILL.md) skill, PR review expectations by [reviewing](.agents/skills/reviewing/SKILL.md).

## Editing these instructions

Each `AGENTS.md` has a byte-identical `CLAUDE.md` twin beside it; the pairs have no primary — edit either side and `scripts/sync_agent_instructions.py` (or the self-healing `agent-instructions-sync` hook) keeps them equal. Word ceilings live in `.hdsh/docs.manifest.json` (`docBudgets`; root 800, `backend/AGENTS.md` 500, `frontend/AGENTS.md` 300, `e2e/AGENTS.md` 150): on red, relocate to the owning tier, then condense; raise a ceiling last with a justified manifest diff. Skills live once in `.agents/skills/` and mirror byte-identically into `.zcode/skills/` and `.claude/skills/`.
