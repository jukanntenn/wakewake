import { BasePage } from './BasePage'

export class DevicesPage extends BasePage {
  async goto(): Promise<void> {
    await super.goto('/dashboard/devices')
    await this.waitForLoad()
  }

  /** 点"创建"按钮打开 DeviceForm Dialog。 */
  async openCreateDialog(): Promise<void> {
    await this.page.getByRole('button', { name: /^_create$|create/i }).first().click()
  }

  /** 填写设备表单（用 placeholder 定位，DeviceForm 无 id/htmlFor）。 */
  async fillDeviceForm(input: {
    name: string
    mac: string
    description?: string
  }): Promise<void> {
    // placeholder 来自 i18n，用模糊匹配 name/MAC/description
    await this.page
      .locator('input[placeholder]')
      .nth(0)
      .fill(input.name)
    await this.page
      .locator('input[placeholder]')
      .nth(1)
      .fill(input.mac)
    if (input.description) {
      await this.page
        .locator('textarea[placeholder], input[placeholder]')
        .nth(2)
        .fill(input.description)
    }
  }

  /** 提交设备表单。 */
  async submitDeviceForm(): Promise<void> {
    await this.page.locator('button[type=submit]').click()
  }

  /** 列表中是否存在指定设备名（设备 CRUD UI 断言）。 */
  async hasDeviceName(name: string): Promise<boolean> {
    const body = await this.page.locator('body').textContent()
    return body?.includes(name) ?? false
  }

  /** 点设备的编辑按钮（aria-label=edit）。 */
  async clickEdit(deviceName: string): Promise<void> {
    // 设备卡片 → 编辑按钮
    const card = this.page.locator(`text=${deviceName}`).locator('..')
    await card.getByRole('button', { name: /edit/i }).click()
  }

  /** 点设备的删除按钮（aria-label=delete）。 */
  async clickDelete(deviceName: string): Promise<void> {
    const card = this.page.locator(`text=${deviceName}`).locator('..')
    await card.getByRole('button', { name: /delete/i }).click()
  }
}
