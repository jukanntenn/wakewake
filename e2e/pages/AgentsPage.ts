import { BasePage } from './BasePage'

export class AgentsPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/agents')
    await this.waitForLoad()
  }

  /** 取 agent 状态文本（pending/online/offline）。 */
  async getStatusText(): Promise<string> {
    const body = (await this.page.locator('body').textContent()) ?? ''
    if (/online/i.test(body)) return 'online'
    if (/offline/i.test(body)) return 'offline'
    return 'pending'
  }

  /** 取 pairing_code 显示文本（pending 完整 / online,offline 脱敏 ****1234）。 */
  async getPairingCodeText(): Promise<string> {
    // 配对码通常在 <code> 或特定文本块
    const codeEl = this.page.locator('code, [data-testid=pairing-code]').first()
    return (await codeEl.textContent()) ?? ''
  }
}
