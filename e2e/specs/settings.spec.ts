/**
 * 场景 1：用户↔server（无 agent）—— 设置（e2e.md §7.2 settings.spec.ts P1）。
 *
 * 3 用例：修改密码/语言切换持久化/主题切换持久化。
 */
import { test, expect } from '../fixtures'
import { createBrowserClient, refreshTokens } from '../utils/api'

test.describe('设置（场景 1）', () => {
  test('修改密码后旧 refresh 失效', async ({ freshUser }) => {
    // 用 freshUser（独立用户）避免污染 globalSetup 的 registeredUser 密码
    const client = createBrowserClient(freshUser.accessToken)
    await client.post('me/password', {
      json: {
        current_password: freshUser.password,
        new_password: 'BrandNew123!',
      },
    })
    // 旧 refresh 失效（改密吊销所有 refresh）
    await expect(
      refreshTokens(createBrowserClient(), freshUser.refreshToken),
    ).rejects.toThrow()
  })

  test('主题切换持久化（data-testid=theme-toggle/dark）', async ({ page, settingsPage }) => {
    await settingsPage.goto()
    // 切换到深色（next-themes 写 localStorage + html class）
    const beforeClass = await page.evaluate(() => document.documentElement.className)
    await settingsPage.toggleTheme('dark')
    await page.waitForTimeout(800)
    // 刷新仍保持（localStorage 持久化）
    await page.reload()
    await page.waitForLoadState('networkidle')
    const afterClass = await page.evaluate(() => document.documentElement.className)
    // dark class 应出现（next-themes 加 .dark 到 html）或 class 变化
    const changed = beforeClass !== afterClass || afterClass.includes('dark')
    expect(changed).toBeTruthy()
  })

  test('语言切换持久化（data-testid=language-switcher）', async ({ page, settingsPage }) => {
    await settingsPage.goto()
    const beforeLang = await page.evaluate(() => document.documentElement.lang)
    // 切到中文（或反向）。locale 存 localStorage key='locale'（i18n/constants.ts）
    const target = beforeLang?.startsWith('zh') ? 'en' : 'zh'
    await settingsPage.selectLanguage(target)
    await page.waitForTimeout(800)
    await page.reload()
    await page.waitForLoadState('networkidle')
    const afterLang = await page.evaluate(() => document.documentElement.lang)
    // lang 属性变化 或 localStorage 'locale' 变化
    const locale = await page.evaluate(() => localStorage.getItem('locale'))
    const changed = beforeLang !== afterLang || locale === target
    expect(changed).toBeTruthy()
  })
})
