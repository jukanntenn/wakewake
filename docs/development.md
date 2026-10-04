# Development guide

English | [中文](development.zh.md)

The setup tutorial takes a new contributor from prerequisites to a checked checkout. The contributor reference that follows covers the daily workflow and the repository's tool integrations.

## Setup tutorial

### Prerequisites

- Rust 1.95+ (edition 2021 workspace) via [rustup](https://rustup.rs)
- Node.js 20+ with [pnpm](https://pnpm.io) for the frontend and e2e workspaces
- Python 3.12+ for dev orchestration, [uv](https://docs.astral.sh/uv/) hosting the `hdsh` gates, and Docker with compose for PostgreSQL 17 + Mailpit

### First-time setup

`python3 devops/dev.py start` brings up the Docker infra (postgres + mailpit), the backend (`cargo watch`), and the frontend (`pnpm dev`), all health-checked; it generates `backend/config.local.toml` on first run. Setup is complete when dev.py reports every service healthy. `dev.py init` starts infra only; `dev.py stop` stops it.

## Contributor reference

### Project layout

One name identifies one concept across the directory, the interface, and any id prefix; the full component map is [architecture.md](architecture.md).

### Daily commands

- `prek run <hook> [--all-files]` — the repo gate set, grouped format / lint / check
- `cargo test -p <crate>` — backend unit and integration tests against real PostgreSQL
- `pnpm -C frontend lint` / `pnpm -C frontend test:run` — frontend static checks and unit tests
- `hdsh pairing verify`, `hdsh rfc verify`, `hdsh adopt verify` — the harness gates; their corpora are configured in `.hdsh/docs.manifest.json` and `.hdsh/pairing.manifest.json` per the [bilingual documentation contract](i18n/README.md)
- `python3 docker/build.py` and `ansible-playbook devops/ansible/deploy.yml -l <env>` — image build and environment deploys

### TODO markers

No urgency-graded TODO convention exists; open an issue or RFC instead of leaving TODO markers in the tree.
