import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const agents = [
  {
    id: 10,
    user_id: 2,
    user_email: 'alice@x.com',
    aid: 'aabbccdd-0000-0000-0000-000000000001',
    name: 'Home Agent',
    status: 'online' as const,
    last_seen: '2026-08-03T00:00:00Z',
    created_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 11,
    user_id: 3,
    user_email: 'bob@x.com',
    aid: 'aabbccdd-0000-0000-0000-000000000002',
    name: 'Office Agent',
    status: 'offline' as const,
    last_seen: '2026-07-01T00:00:00Z',
    created_at: '2026-01-02T00:00:00Z',
  },
]

describe('Admin agents page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('渲染 agent 表 + 归属邮箱 + 状态', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminAgents: () => ({
        data: { items: agents, page: 1, page_size: 20, total: 2 },
        isLoading: false,
      }),
      useDisconnectAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('Home Agent')).toBeInTheDocument()
    expect(screen.getByText('Office Agent')).toBeInTheDocument()
    expect(screen.getByText('alice@x.com')).toBeInTheDocument()
    expect(screen.getByText('Online')).toBeInTheDocument()
    expect(screen.getByText('Offline')).toBeInTheDocument()
  })

  it('online agent 的 Disconnect 按钮可点，offline 的 disabled', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminAgents: () => ({
        data: { items: agents, page: 1, page_size: 20, total: 2 },
        isLoading: false,
      }),
      useDisconnectAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    const disconnectBtns = screen.getAllByText('Disconnect')
    // online agent (Home) → enabled；offline (Office) → disabled
    expect(disconnectBtns[0]).toBeEnabled()
    expect(disconnectBtns[1]).toBeDisabled()
  })

  it('空数据显示 emptyAgents', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminAgents: () => ({
        data: { items: [], page: 1, page_size: 20, total: 0 },
        isLoading: false,
      }),
      useDisconnectAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('No agents found.')).toBeInTheDocument()
  })

  it('加载中显示 loading', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminAgents: () => ({ data: undefined, isLoading: true }),
      useDisconnectAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })
})
