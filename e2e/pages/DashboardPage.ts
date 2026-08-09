import { BasePage } from './BasePage'

export class DashboardPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard')
    await this.waitForLoad()
  }

  /** 导航到各子页（侧边栏 Link）。 */
  async navTo(path: string): Promise<void> {
    await this.page.goto(path)
    await this.waitForLoad()
  }
}
