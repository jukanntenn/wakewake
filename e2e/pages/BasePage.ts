/**
 * BasePage（e2e.md §3.2 POM）。
 * goto/waitForLoad/screenshot 等通用方法。
 */
import type { Page, Locator } from '@playwright/test'

export class BasePage {
  constructor(protected readonly page: Page) {}

  /** baseURL 相对路径导航。 */
  async goto(path: string): Promise<void> {
    await this.page.goto(path)
  }

  /** 等待页面 networkidle（SPA 水合完成）。 */
  async waitForLoad(): Promise<void> {
    await this.page.waitForLoadState('networkidle')
  }

  /** 截图（失败诊断）。 */
  async screenshot(name: string): Promise<void> {
    await this.page.screenshot({ path: `test-results/${name}.png`, fullPage: true })
  }

  /** testid 优先定位（e2e.md §7.7）。 */
  protected byTestid(id: string): Locator {
    return this.page.getByTestId(id)
  }

  /** 当前 URL 路径。 */
  currentPath(): string {
    return new URL(this.page.url()).pathname
  }
}
