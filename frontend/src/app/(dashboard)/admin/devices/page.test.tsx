import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const devices = [
  {
    did: 'aabbccddeeff0011',
    user_id: 2,
    user_email: 'alice@x.com',
    name: 'NAS',
    mac_display: 'AA:**:**:**:**:FF',
    description: null,
    cloud_status: 'error' as const,
    last_error: 'createTopic timeout',
    last_drift_at: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-08-01T00:00:00Z',
  },
  {
    did: '1122334455667788',
    user_id: 3,
    user_email: 'bob@x.com',
    name: 'PC',
    mac_display: '11:**:**:**:**:88',
    description: null,
    cloud_status: 'synced' as const,
    last_error: null,
    last_drift_at: null,
    created_at: '2026-01-02T00:00:00Z',
    updated_at: '2026-08-02T00:00:00Z',
  },
]

describe('Admin devices page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('渲染设备表 + 归属邮箱 + Resync 按钮', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminDevices: () => ({
        data: { items: devices, page: 1, page_size: 20, total: 2 },
        isLoading: false,
      }),
      useResyncDevice: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('NAS')).toBeInTheDocument()
    expect(screen.getByText('PC')).toBeInTheDocument()
    expect(screen.getByText('alice@x.com')).toBeInTheDocument()
    // 两行各一个 Resync
    expect(screen.getAllByText('Resync')).toHaveLength(2)
  })

  it('sync_error 行渲染红字 Sync error 状态', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminDevices: () => ({
        data: { items: devices, page: 1, page_size: 20, total: 2 },
        isLoading: false,
      }),
      useResyncDevice: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // device-sync-v3：页面直接渲染 cloud_status 字符串值（error / synced）
    expect(screen.getAllByText('error').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('synced').length).toBeGreaterThanOrEqual(1)
    // last_error 展示
    expect(screen.getByText('createTopic timeout')).toBeInTheDocument()
  })

  it('空数据显示 emptyDevices 文案', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminDevices: () => ({
        data: { items: [], page: 1, page_size: 20, total: 0 },
        isLoading: false,
      }),
      useResyncDevice: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('No devices found.')).toBeInTheDocument()
  })

  it('加载中显示 loading', async () => {
    vi.doMock('@/hooks/useAdmin', () => ({
      useAdminDevices: () => ({ data: undefined, isLoading: true }),
      useResyncDevice: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })
})
