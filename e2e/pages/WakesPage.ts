import { BasePage } from './BasePage'

export class WakesPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/wakes')
    await this.waitForLoad()
  }

  /** 是否有唤醒记录包含指定设备名。 */
  async hasWakeForDevice(deviceName: string): Promise<boolean> {
    const body = await this.page.locator('body').textContent()
    return body?.includes(deviceName) ?? false
  }
}
