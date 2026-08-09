import { defineConfig, devices } from '@playwright/test'

// Node.js fetch（globalSetup + ky 客户端）不信任 Caddy tls internal 自签名证书。
// Playwright 的 ignoreHTTPSErrors 只作用于浏览器导航，不覆盖 Node fetch。
// 设 NODE_TLS_REJECT_UNAUTHORIZED=0 让 Node fetch 也跳过证书校验（E2E 环境专用，
// 与浏览器 ignoreHTTPSErrors 等价）。
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0'

/**
 * Playwright 配置（e2e.md §3.3）。
 *
 * 关键决策（e2e.md §11.2 修订记录）：
 * - globalSetup 用路径字符串（非 require.resolve）——ESM 顶层 require 不可用。
 * - storageState 写 access_token（覆盖生产 partialize 行为——生产只 persist refresh_token）。
 * - 默认复用登录态，anonymous project 用 storageState: undefined 不加载。
 * - CI workers=1 串行避免容器竞态（参考项目 bugfix 教训）。
 */
export default defineConfig({
  testDir: './specs',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1, // agent 容器是单例，必须串行（本地 + CI 一致）
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['json', { outputFile: 'test-results/results.json' }],
    process.env.CI ? ['github'] : ['list'],
  ],
  use: {
    baseURL: process.env.BASE_URL ?? 'https://localhost:8443',
    ignoreHTTPSErrors: true, // tls internal 自签名
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    storageState: '.auth/user.json', // 默认复用登录态（§5）
  },
  projects: [
    // 未登录：登录/注册页测试（不加载 storageState）
    {
      name: 'anonymous',
      use: { ...devices['Desktop Chrome'], storageState: undefined },
    },
    // 默认登录（globalSetup 注册的用户）
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    // admin（DB 直改 is_superuser=true）
    {
      name: 'admin',
      use: {
        ...devices['Desktop Chrome'],
        storageState: '.auth/admin.json',
      },
    },
  ],
  // 注意：路径字符串，不用 require.resolve（ESM 冲突，e2e.md §3.3/§11.2）
  globalSetup: './fixtures/global-setup.ts',
})
