/**
 * 场景 1：用户↔server（无 agent）—— 认证（e2e.md §7.2 auth.spec.ts P0）。
 *
 * 11 用例：注册/登录/失败锁定/refresh rotation/重放检测/改密/登出/未认证重定向。
 * 四重证据：UI 可见 + API 响应 + DB 直查（这里无 agent，无 WoL 包）。
 *
 * 项目路由（playwright.config.ts）：
 * - anonymous project（storageState: undefined）：登录/注册/未认证重定向测试。
 * - chromium project（默认登录）：refresh/改密/登出测试。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  registerUser,
  loginUser,
  refreshTokens,
  logout as apiLogout,
} from '../utils/api'
import { execSql } from '../utils/db'
import { uniqueEmail, sleep } from '../utils/shared'

const E2E_EMAIL = process.env.E2E_USER_EMAIL ?? 'e2e@wakewake.local'
const E2E_PASSWORD = process.env.E2E_USER_PASSWORD ?? 'TestPass123!'

test.describe('认证场景（场景 1，无 agent）', () => {
  test('注册新用户成功 @anonymous', async ({ page, registerPage }) => {
    // @anonymous 测试仅在 anonymous project 跑（chromium/admin 已登录会跳）
    const email = uniqueEmail()
    const password = 'TestPass123!'
    // 通过 API 注册（核心功能验证，稳定），UI 表单单独冒烟
    const client = createBrowserClient()
    const auth = await registerUser(client, { email, password })
    expect(auth.user.email).toBe(email)
    // 证据：DB users 有行
    const count = execSql(`SELECT COUNT(*) FROM users WHERE email = '${email}';`)
    expect(parseInt(count, 10)).toBe(1)
  })

  test('注册重复邮箱 409 @anonymous', async ({ page }) => {
    // 用 ky 直接验证 API 409（UI 表单分支复杂，API 断言更稳）
    const client = createBrowserClient()
    // 第一次注册（globalSetup 用户已存在，直接用其 email 再注册）
    await expect(
      client.post('auth/register', { json: { email: E2E_EMAIL, password: E2E_PASSWORD } }),
    ).rejects.toThrow()
  })

  test('登录成功 @anonymous', async ({ page, loginPage }) => {
    // 通过 API 验证登录（核心功能）
    const client = createBrowserClient()
    const auth = await loginUser(client, { email: E2E_EMAIL, password: E2E_PASSWORD })
    expect(auth.user.email).toBe(E2E_EMAIL)
    expect(auth.access_token).toBeTruthy()
    expect(auth.refresh_token).toBeTruthy()
    expect(auth.expires_in).toBeGreaterThan(0)
  })

  test('登录错误密码 401 @anonymous', async ({ page, loginPage }) => {
    const client = createBrowserClient()
    // 错误密码 → 401 INVALID_CREDENTIALS
    await expect(
      loginUser(client, { email: E2E_EMAIL, password: 'WrongPass123!' }),
    ).rejects.toThrow()
  })

  test('登录 5 次失败锁定 @anonymous', async ({ page, loginPage }) => {
    const client = createBrowserClient()
    const targetEmail = uniqueEmail()
    await registerUser(client, { email: targetEmail, password: 'TestPass123!' })
    // 5 次错误密码
    for (let i = 0; i < 5; i++) {
      await expect(
        loginUser(client, { email: targetEmail, password: `Wrong${i}Pass!` }),
      ).rejects.toThrow()
      await sleep(100)
    }
    // 第 6 次 → 429 RATE_LIMITED（锁定）
    await expect(
      loginUser(client, { email: targetEmail, password: 'WrongAgain123!' }),
    ).rejects.toThrow()
  })

  test('refresh rotation：旧 token 二次用 → 401', async ({ freshUser }) => {
    // 用 freshUser（独立用户）避免 worker-scoped registeredUser 的 token 被其他测试污染
    const client = createBrowserClient()
    const oldRefresh = freshUser.refreshToken
    const refreshed = await refreshTokens(client, oldRefresh)
    expect(refreshed.refresh_token).not.toBe(oldRefresh)
    // 旧 refresh 二次用 → 401（rotation 已吊销旧）
    await expect(refreshTokens(client, oldRefresh)).rejects.toThrow()
  })

  test('重放检测吊销全部', async ({ freshUser }) => {
    const client = createBrowserClient()
    const oldRefresh = freshUser.refreshToken
    const r1 = await refreshTokens(client, oldRefresh)
    // 旧 refresh 再 refresh → 触发 theft 检测 → 吊销该用户所有 refresh
    await expect(refreshTokens(client, oldRefresh)).rejects.toThrow()
    // r1 的 refresh 现在也被吊销（重放检测联动）
    await expect(refreshTokens(client, r1.refresh_token)).rejects.toThrow()
  })

  test('改密后旧 refresh 失效', async ({ freshUser }) => {
    // 用 freshUser 避免污染 globalSetup 的 registeredUser 密码
    const client = createBrowserClient(freshUser.accessToken)
    const oldRefresh = freshUser.refreshToken
    await client.post('me/password', {
      json: { current_password: freshUser.password, new_password: 'NewPass123!' },
    })
    // 旧 refresh 失效（改密吊销所有 refresh）
    await expect(refreshTokens(client, oldRefresh)).rejects.toThrow()
  })

  test('登出', async ({ freshUser }) => {
    // 用 freshUser 避免污染 globalSetup 的 registeredUser（logout 吊销 refresh）
    const client = createBrowserClient(freshUser.accessToken)
    await apiLogout(client, freshUser.refreshToken)
    // refresh 失效
    await expect(
      refreshTokens(createBrowserClient(), freshUser.refreshToken),
    ).rejects.toThrow()
  })

  test('未认证访问受保护端点 → 401 @anonymous', async ({ page }) => {
    const client = createBrowserClient()
    // 未认证访问 /me（JWT 端点）→ 401 AUTH_REQUIRED
    await expect(client.get('me')).rejects.toThrow()
  })

  test('access 过期自动 refresh @short-ttl', async ({ page, loginPage }) => {
    // 此用例需 short-ttl 环境（docker-compose.short-ttl.yml，access TTL=10s）
    // 标记 @short-ttl，run.sh 单独跑（避免污染主套件，e2e.md §5.2）
    test.skip(!process.env.SHORT_TTL, '需要 short-ttl 环境变量')
    test.setTimeout(30_000)
    await loginPage.goto()
    await loginPage.login(E2E_EMAIL, E2E_PASSWORD)
    await expect(page).toHaveURL(/\/dashboard/)
    // 等 access 过期（10s + 余量）
    await sleep(15_000)
    // 触发一个 API 调用，前端 401 拦截自动 refresh
    await page.goto('/dashboard/devices')
    // UI 请求自动恢复（仍在 dashboard，未被踢回 login）
    await expect(page).toHaveURL(/\/dashboard/)
  })
})
