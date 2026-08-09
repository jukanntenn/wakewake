import { BasePage } from './BasePage'

export class IntegrationsPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/integrations')
    await this.waitForLoad()
  }

  /** 是否已连接 bemfa 集成（UI 显示 connected）。 */
  async isBemfaConnected(): Promise<boolean> {
    const body = await this.page.locator('body').textContent()
    return /connected|enabled/i.test(body ?? '')
  }
}
