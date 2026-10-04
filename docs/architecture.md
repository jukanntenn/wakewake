# wakewake architecture

English | [中文](architecture.zh.md)

Read this before changing the source tree. It is the ordered map of the codebase — components, their boundaries, and where new behavior goes; decision rationale lives in the linked RFCs.

## What this package is

wakewake ships a wake-on-LAN service: a Rust backend (axum + sqlx + tokio) with a standalone WoL agent binary, plus a statically exported Next.js 16 frontend, deployed together as a single multi-arch Docker image behind Caddy. The operator runs it on a 2 GB VPS sized for 100k concurrent SSE connections; end users wake and manage machines through the web UI, while agents stay connected to the backend over SSE.

## Components

| Component | Responsibility | Public surface |
|---|---|---|
| `backend/crates/protocol` | Wire types shared by server and agent | Rust crate `protocol` |
| `backend/crates/server` | HTTP API, SSE hub, persistence | REST endpoints + SSE stream |
| `backend/crates/agent` | Standalone wake client on target machines | binary `wakewake-agent` |
| `backend/crates/seed` | Load-test seeding | Rust crate `seed` |
| `frontend/` | Web UI (static export) | served static site |
| `docker/` `devops/` | Image build; dev environment and deploys | `docker/build.py`, `dev.py`, ansible |
| `.agents/rfcs/` `.agents/skills/` | Decision records; agent workflows | hdsh gates and skills |

## Where new behavior goes

API or hub behavior goes in `backend/crates/server`; wire-format changes start in `protocol`; agent-side behavior in `backend/crates/agent`; UI in `frontend/`; schema changes land in `backend/migrations` as reversible `sqlx migrate add -r` pairs. Every non-trivial change adds or updates a decision record under `.agents/rfcs/`. The owning reference documents are the [documentation standard](AGENTS.md), the [bilingual documentation contract](i18n/README.md), and the [RFC rules](../.agents/rfcs/README.md).

The contributor entry points are in [development.md](development.md).
