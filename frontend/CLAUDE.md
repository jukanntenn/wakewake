# frontend/AGENTS.md

Next.js 16 + React 19 + TypeScript app, static export (`output: "export"`) — no Node.js runtime in production; Caddy serves `out/` as flat files.

## Commands (`frontend/`)

- `pnpm dev` — dev server (port `${FRONTEND_PORT:-3034}`)
- `pnpm build` — static export to `out/`
- `pnpm lint` — ESLint (eslint-config-next)
- `pnpm format` / `pnpm format:check` — Prettier write / check (config in `.prettierrc.json`)
- `pnpm test:run` — Vitest (jsdom)

## Style and constraints

- Function components + hooks only; server components are not available (static export).
- Every user-facing string goes through `next-intl` (`messages/*.json`, 8 locales: en, zh, ja, ko, de, fr, es, pt) — a string change updates all locale files in the same change.
- Static export means no server-side anything: no API routes, no middleware, no dynamic server rendering; browser-side Web Crypto covers MAC encryption (requires a secure context).

## Testing

Vitest + Testing Library (jsdom) for units; UI work is verified with playwright-cli screenshots before finishing (see the [iterating](../.agents/skills/iterating/SKILL.md) skill).
