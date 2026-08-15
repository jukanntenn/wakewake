---
name: commit
description: Use when the user asks to commit or stage changes (commit/stage/save/submit), when a task ends with dirty files to commit, or when multiple files should be split into logical commits.
---

# Commit

Group by logical change, not by file. Draft a plan, confirm, then execute. Never push, never amend.

1. `git status --porcelain` + `git log --oneline -5` for current changes and history style.
2. Separate AI-edited files from unrecognized ones; list unrecognized separately, never mix them in.
3. Group by logical unit (route+service+repo, page+hook+types; a migration's `.up.sql`/`.down.sql` never split); order: `build/chore` → `feat` → `fix` → `refactor` → `style` → `docs` → `test`.
4. Present the plan once; after confirmation run `git add` + `git commit` batch by batch. Rejected → stop.
5. All gates live in prek — never `--no-verify`. The pre-commit hooks (fmt / prettier / hygiene) and the commit-msg format check fire automatically on every `git commit`. Run the touched project's slow gates yourself before finishing, from the repo root: `prek run --all-files --hook-stage pre-push backend:test` / `frontend:test`, or single hooks (`backend:clippy`, `frontend:lint`, `frontend:typecheck`). `backend:test` uses a real PostgreSQL and falls back to `cargo test --lib` with a loud warning when none is reachable (`python3 devops/dev.py init` brings one up).
6. Single file → skip the plan, commit directly.

Message: `<type>(<scope>): <desc>` — lowercase, imperative, no trailing period. Types: `feat`/`fix`/`refactor`/`docs`/`test`/`chore`/`ci`/`build`/`style`/`perf`/`revert` (the commit-msg hook enforces this list). Scopes: `backend`/`frontend`/`agent`/`protocol`/`devops`/`docker`/`e2e`/`i18n` (omit for cross-cutting). Match the change's language.

- Generated files (`Cargo.lock`, `pnpm-lock.yaml`) bundle into the producing commit, or as a standalone `chore` — regenerate, never hand-edit.
- Migration + consuming code, config template + code (`config.example.toml` + `config.rs`), and bilingual docs (`README.md` + `README_zh.md`, likewise the whole `messages/*.json` set) stay together when the code depends on them.
- `AGENTS.md` edits: run `python3 scripts/sync_agents.py` in the same commit, or the agents-sync hook blocks it.
- Never silently include unrecognized files. Never amend, never push, never placeholder messages (`tmp commit~`, `wip`, `update files`).
