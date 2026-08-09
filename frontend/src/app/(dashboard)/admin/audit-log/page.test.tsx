import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const entries = [
  {
    id: 1,
    actor_id: 1,
    actor_email: 'admin@x.com',
    action: 'user.disable',
    target_user_id: 5,
    target_agent_id: null,
    target_device_did: null,
    detail: { forced: true },
    created_at: '2026-08-03T10:00:00Z',
  },
  {
    id: 2,
    actor_id: 1,
    actor_email: 'admin@x.com',
    action: 'device.resync',
    target_user_id: null,
    target_agent_id: null,
    target_device_did: 'aabbccdd',
    detail: {},
    created_at: '2026-08-03T11:00:00Z',
  },
]

describe('Admin audit-log page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('渲染审计条目（actor / action / target）', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAuditLog: () => ({
        data: { items: entries, page: 1, page_size: 30, total: 2 },
        isLoading: false,
      }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 两行 actor_email（两行 + select option 不含），至少出现
    expect(screen.getAllByText('admin@x.com').length).toBeGreaterThanOrEqual(2)
    // action 既在表格行又在 select option → getAllByText
    expect(screen.getAllByText('user.disable').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('device.resync').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText(/user#5/)).toBeInTheDocument()
  })

  it('空审计显示 emptyAudit', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAuditLog: () => ({
        data: { items: [], page: 1, page_size: 30, total: 0 },
        isLoading: false,
      }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('No audit entries yet.')).toBeInTheDocument()
  })

  it('加载中显示 loading', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAuditLog: () => ({ data: undefined, isLoading: true }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('多页时显示分页控件', async () => {
    // 60 条 → 2 页（PAGE_SIZE=30）
    vi.doMock('@/hooks/useAdmin', () => ({
      useAuditLog: () => ({
        data: { items: entries, page: 1, page_size: 30, total: 60 },
        isLoading: false,
      }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText(/Page 1 \/ 2/)).toBeInTheDocument()
  })
})
