import { BasePage } from './BasePage'

export class SettingsPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/settings')
    await this.waitForLoad()
  }

  /** 填写改密表单（placeholder 定位 currentPassword/newPassword）。 */
  async fillChangePassword(current: string, next: string): Promise<void> {
    await this.page.getByPlaceholder(/current.?password/i).fill(current)
    await this.page.getByPlaceholder(/new.?password/i).fill(next)
  }

  /** 提交改密。 */
  async submitChangePassword(): Promise<void> {
    await this.page.locator('button[type=submit]').click()
  }

  /** 切换主题（深色）。data-testid wrapper 在 ThemeToggle 组件外层 div。 */
  async toggleTheme(target: 'light' | 'dark' | 'system' = 'dark'): Promise<void> {
    // 点 wrapper 内的按钮（Menu.Trigger 渲染为 button）
    await this.page.getByTestId('theme-toggle-wrapper').locator('button').click()
    await this.page.getByTestId(`theme-${target}`).click()
  }

  /** 切换语言（data-testid wrapper 在 LanguageSwitcher 组件外层 div）。 */
  async selectLanguage(code: string): Promise<void> {
    await this.page.getByTestId('language-switcher-wrapper').locator('button').click()
    await this.page.getByTestId(`lang-${code}`).click()
  }
}
