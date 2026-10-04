# e2e/AGENTS.md

Playwright end-to-end suite (chromium only), a separate pnpm workspace driving the full container stack — never mocks.

## Commands (`e2e/`)

- `pnpm test` — `pretest` builds and starts the full stack via `docker-compose.e2e.yml` (app + postgres + mailpit + bemfa-mock + agent), waits for health, runs `playwright test`; `posttest` tears it down (`down -v`).
- `pnpm test:headed` / `pnpm test:debug` / `pnpm report` — headed run, debugger, last HTML report.

## Conventions

- Specs live in `specs/*.spec.ts`; fixtures and helpers in `fixtures/`, `utils/`, `pages/`.
- `workers = 1` — the agent container is a singleton; tests run serially locally and in CI alike.
- The stack uses built-in test secrets and self-signed TLS (`NODE_TLS_REJECT_UNAUTHORIZED=0` for Node fetch); never reuse test credentials with production.
