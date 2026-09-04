import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderWithProviders } from '@/test/test-utils'

// 横幅受众规则（admin-risk-controls WRFC）：横幅挂在登录后的 dashboard，
// 仅 readonly 档展示——registration_disabled 不影响存量用户（其受众在注册页），
// full 档用户被拦截根本看不到。
let mockMaintenance: { enabled: boolean; mode: string; message: string } | undefined
vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-query')>()
  return { ...actual, useQuery: () => ({ data: mockMaintenance }) }
})

async function renderBanner() {
  const { MaintenanceBanner } = await import('./maintenance-banner')
  return renderWithProviders(<MaintenanceBanner />)
}

describe('MaintenanceBanner 档位可见性', () => {
  beforeEach(() => {
    mockMaintenance = undefined
  })

  it('维护关闭 → 不渲染', async () => {
    mockMaintenance = { enabled: false, mode: 'registration_disabled', message: '' }
    const { container } = await renderBanner()
    expect(container.firstChild).toBeNull()
  })

  it('registration_disabled → 不渲染（投错受众）', async () => {
    mockMaintenance = { enabled: true, mode: 'registration_disabled', message: 'paused' }
    const { container } = await renderBanner()
    expect(container.firstChild).toBeNull()
  })

  it('readonly → 渲染 message', async () => {
    mockMaintenance = { enabled: true, mode: 'readonly', message: 'read-only now' }
    const { container } = await renderBanner()
    expect(container.textContent).toContain('read-only now')
  })

  it('full → 不渲染（用户被拦截看不到）', async () => {
    mockMaintenance = { enabled: true, mode: 'full', message: 'down' }
    const { container } = await renderBanner()
    expect(container.firstChild).toBeNull()
  })

  it('readonly 且无 message → 显示默认横幅文案', async () => {
    mockMaintenance = { enabled: true, mode: 'readonly', message: '' }
    const { container } = await renderBanner()
    expect(container.textContent).toContain('Scheduled maintenance in progress')
  })
})
