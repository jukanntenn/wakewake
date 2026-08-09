# wakewake frontend

Next.js 16 + React 19 single-page app for [wakewake](../README.md) — a
self-hosted Wake-on-LAN service. Static-exported (`output: "export"`) and
served as flat files by Caddy; no Node.js runtime in production.

## Stack

- [Next.js 16](https://nextjs.org) (App Router, `output: "export"`)
- React 19, TypeScript, Tailwind CSS 4
- Zustand (auth store), TanStack Query (server state)
- [next-intl](https://next-intl-docs.vercel.app) (8 locales: en, zh, ja, ko, de, fr, es, pt)
- @base-ui/react, react-hook-form, sonner, lucide-react

## Commands

```bash
pnpm install
pnpm dev          # dev server on ${FRONTEND_PORT:-3034}
pnpm build        # static export → out/
pnpm test:run     # Vitest (jsdom + v8 coverage)
pnpm lint         # ESLint (eslint-config-next)
pnpm format       # Prettier write
```

See the root [`README.md`](../README.md) and [`AGENTS.md`](../AGENTS.md) for
the full architecture, deployment, and contribution guide.
