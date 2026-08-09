import type { NextConfig } from 'next'

// 纯静态导出（build.md §1）：output: 'export'，运行时零 Node 进程。
// 不用 NEXT_PUBLIC_ 变量（构建时冻死，与镜像复用冲突，build.md §4）——前端发相对路径，Caddy 反代。
// 不用 next-intl/plugin（服务端 i18n 装配，纯静态导出不可用，build.md §三）——改为纯客户端 LocaleProvider。
// 不配 webpack 字段（Next.js 16 下会致 Turbopack 构建失败，build.md §2.2）。
const apiTarget = process.env.API_PROXY_TARGET || 'http://localhost:8080'

const nextConfig: NextConfig = {
  output: 'export',
  reactStrictMode: true,
  // rewrites 仅 next dev 生效，next build (output: export) 自动忽略，产物纯净（build.md §五）。
  rewrites: async () => [
    {
      source: '/api/:path*',
      destination: `${apiTarget}/api/:path*`,
    },
  ],
}

export default nextConfig
