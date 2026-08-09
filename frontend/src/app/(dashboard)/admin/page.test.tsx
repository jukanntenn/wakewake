import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

// Mock sonner
vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

// Mock useAuthStore（overview 不依赖，但 layout 可能引用）
vi.mock('@/stores/auth', () => ({
  useAuthStore: Object.assign(vi.fn(), {
    getState: () => ({ token: null, refreshToken: null, user: { id: 1, is_superuser: true } }),
  }),
}))

const statsData = {
  users: 5,
  active_users: 4,
  devices: 3,
  agents: 2,
  integrations: 1,
  wakes: 10,
  devices_syncing: 1,
  devices_sync_error: 0,
  online_agents: 1,
}

describe('Admin overview page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('渲染 stats 卡片 + 各计数', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminStats: () => ({ data: statsData, isLoading: false }),
      useAuditLog: () => ({ data: { items: [], page_size: 10, total: 0 } }),
    }))
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    // statUsers: 'Total users' 旁边渲染 5
    expect(screen.getByText('5')).toBeInTheDocument()
    expect(screen.getByText('10')).toBeInTheDocument() // wakes
    expect(screen.getByText('Total users')).toBeInTheDocument()
  })

  it('devices_sync_error > 0 时显示 attention 告警卡', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminStats: () => ({ data: { ...statsData, devices_sync_error: 2 }, isLoading: false }),
      useAuditLog: () => ({ data: { items: [], page_size: 10, total: 0 } }),
    }))
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByText('attention')).toBeInTheDocument()
  })

  it('加载中显示 loading 文案', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminStats: () => ({ data: undefined, isLoading: true }),
      useAuditLog: () => ({ data: undefined }),
    }))
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('最近审计日志渲染', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminStats: () => ({ data: statsData, isLoading: false }),
      useAuditLog: () => ({
        data: {
          items: [
            {
              id: 1,
              actor_id: 1,
              actor_email: 'admin@x.com',
              action: 'user.disable',
              target_user_id: 5,
              target_agent_id: null,
              target_device_did: null,
              detail: {},
              created_at: '2026-08-03T00:00:00Z',
            },
          ],
          page_size: 10,
          total: 1,
        },
      }),
    }))
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByText('admin@x.com')).toBeInTheDocument()
    expect(screen.getByText('user.disable')).toBeInTheDocument()
    expect(screen.getByText(/user#5/)).toBeInTheDocument()
  })
})
