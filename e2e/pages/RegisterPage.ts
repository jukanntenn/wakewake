import { BasePage } from './BasePage'

export class RegisterPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/register')
    await this.waitForLoad()
  }

  async fill(email: string, password: string): Promise<void> {
    await this.page.locator('#email').fill(email)
    await this.page.locator('#password').fill(password)
  }

  async submit(): Promise<void> {
    await this.page.locator('button[type=submit]').click()
  }

  /** 注册并等待跳转 dashboard。 */
  async register(email: string, password: string): Promise<void> {
    await this.fill(email, password)
    await this.submit()
    await this.page.waitForURL(/\/dashboard/, { timeout: 10_000 })
  }
}
