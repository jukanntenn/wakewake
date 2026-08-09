/**
 * 三层 fixture 继承链 第 3 层：认证 fixture（e2e.md §3.4/§5.5）。
 *
 * data.ts base
 *    ↓ extend<AuthFixtures>（auth.ts：storageState 角色）
 *
 * 大多数认证角色由 playwright.config.ts 的 projects（anonymous/chromium/admin）+ storageState
 * 文件处理（globalSetup 写 user.json + admin.json）。本层补 adminUser fixture（DB 直改
 * is_superuser=true，§5.5），供非 admin project 的 spec 用 API client 测 admin 端点。
 */
import { base as dataBase, expect } from './data'
import { createBrowserClient, registerUser, loginUser } from '../utils/api'
import { execSql } from '../utils/db'
import { uniqueEmail } from '../utils/shared'
import type { UserFixture } from './data'

const ADMIN_PASSWORD = 'AdminPass123!'

export const base = dataBase.extend<{
  /** 管理员用户（DB 直改 is_superuser，e2e.md §5.5）。 */
  adminUser: UserFixture
}>({
  adminUser: async ({}, use) => {
    const client = createBrowserClient()
    const email = uniqueEmail('admin-')
    const auth = await registerUser(client, { email, password: ADMIN_PASSWORD })
    // is_superuser 无法通过 API 注册获得（auth_service 硬编码 false），DB 直改
    execSql(`UPDATE users SET is_superuser = true WHERE id = ${auth.user.id};`)
    // 重新登录拿带 is_superuser=true 的 access token
    const reauth = await loginUser(client, { email, password: ADMIN_PASSWORD })
    await use({
      user: reauth.user,
      accessToken: reauth.access_token,
      refreshToken: reauth.refresh_token,
      email,
      password: ADMIN_PASSWORD,
    })
  },
})

export type { UserFixture }
export { expect }
