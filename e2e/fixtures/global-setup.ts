/**
 * globalSetup（e2e.md §5.1）。
 *
 * 注册测试用户 + 用 chromium 建立 storageState（写 localStorage['wakewake-auth']）。
 * storageState 写 access_token（覆盖生产 partialize 行为——生产只 persist refreshToken + user）。
 * E2E 立即可用，过期由前端自动 refresh（真实用户行为）。
 *
 * 关键修正：addInitScript 不在 storageState 快照里捕获 localStorage（origins 为空）。
 * 改用 page.goto + page.evaluate 设置 localStorage，再 storageState 快照（origins 含 localStorage）。
 */
import { chromium, type Browser, type FullConfig } from '@playwright/test'
import { isHTTPError } from 'ky'
import { createBrowserClient, registerUser, loginUser } from '../utils/api'
import { execSql } from '../utils/db'
import { randomUUID } from 'node:crypto'

interface AuthData {
  access_token: string
  refresh_token: string
  user: { id: number; email: string; is_superuser: boolean }
}

/** 写入 wakewake-auth localStorage（zustand persist 格式，含 token 让守卫放行）。 */
async function seedStorageState(browser: Browser, auth: AuthData, statePath: string) {
  const context = await browser.newContext({ ignoreHTTPSErrors: true })
  const page = await context.newPage()
  // 必须先打开页面（同源）才能写 localStorage
  await page.goto(process.env.BASE_URL ?? 'https://localhost:8443/login')
  await page.evaluate((authData) => {
    localStorage.setItem(
      'wakewake-auth',
      JSON.stringify({
        state: {
          token: authData.access_token,
          refreshToken: authData.refresh_token,
          user: authData.user,
          _hasHydrated: true,
        },
        version: 0,
      }),
    )
  }, auth)
  await context.storageState({ path: statePath })
  await context.close()
}

export default async function globalSetup(_config: FullConfig) {
  const client = createBrowserClient()
  const email = process.env.E2E_USER_EMAIL ?? 'e2e@wakewake.local'
  const password = process.env.E2E_USER_PASSWORD ?? 'TestPass123!'

  let auth: AuthData
  try {
    auth = await registerUser(client, { email, password })
  } catch (e) {
    // 已存在则登录（409 USER_EXISTS）。用 isHTTPError 防 cross-realm instanceof 问题。
    if (isHTTPError(e) && e.response.status === 409) {
      // 旧用户可能 email_verified=false（redesign 前注册），login 会被拦截 → DB 直改。
      execSql(`UPDATE users SET email_verified = true WHERE email = '${email}';`)
      auth = await loginUser(client, { email, password })
    } else {
      throw e
    }
  }

  const browser = await chromium.launch()
  await seedStorageState(browser, auth, '.auth/user.json')

  // admin 用户（DB 直改 is_superuser，API 无法注册 superuser，e2e.md §5.5）
  const adminEmail = `admin-${randomUUID()}@e2e.wakewake.local`
  const adminAuth = await registerUser(client, {
    email: adminEmail,
    password: 'AdminPass123!',
  })
  execSql(`UPDATE users SET is_superuser = true WHERE id = ${adminAuth.user.id};`)
  const reauth = await loginUser(client, {
    email: adminEmail,
    password: 'AdminPass123!',
  })

  await seedStorageState(browser, reauth, '.auth/admin.json')
  await browser.close()
}
