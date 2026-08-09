import { BasePage } from './BasePage'

export class AdminUsersPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/admin/users')
    await this.waitForLoad()
  }

  /** 是否显示用户列表（admin 视角）。 */
  async hasUserList(): Promise<boolean> {
    const body = await this.page.locator('body').textContent()
    return /email|users/i.test(body ?? '')
  }
}
