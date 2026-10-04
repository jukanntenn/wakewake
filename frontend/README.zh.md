# wakewake frontend

[English](README.md) | 中文

[wakewake](../README.zh.md) 的 Next.js 16 + React 19 单页应用——一个自托管的 Wake-on-LAN 服务。静态导出（`output: "export"`），由 Caddy 以纯静态文件托管；生产环境无 Node.js 运行时。

## Stack

- [Next.js 16](https://nextjs.org)（App Router，`output: "export"`）
- React 19、TypeScript、Tailwind CSS 4
- Zustand（认证 store）、TanStack Query（服务端状态）
- [next-intl](https://next-intl-docs.vercel.app)（8 语言：en、zh、ja、ko、de、fr、es、pt）
- @base-ui/react、react-hook-form、sonner、lucide-react

## Commands

```bash
pnpm install
pnpm dev          # dev server on ${FRONTEND_PORT:-3034}
pnpm build        # static export → out/
pnpm test:run     # Vitest (jsdom + v8 coverage)
pnpm lint         # ESLint (eslint-config-next)
pnpm format       # Prettier write
```

完整架构、部署与贡献指南见根 [`README.zh.md`](../README.zh.md) 与 [`AGENTS.md`](../AGENTS.md)。
