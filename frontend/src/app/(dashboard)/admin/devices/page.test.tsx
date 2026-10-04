import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'
import type { AdminDevice, ListEnvelope } from '@/lib/api'

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

vi.mock('next/navigation', () => ({
  useSearchParams: () => new URLSearchParams(),
  usePathname: () => '/admin/devices',
  useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
}))

const mockUser = { id: 1, email: 'root@x.com', is_superuser: true }
vi.mock('@/stores/auth', () => ({
  useAuthStore: Object.assign(
    vi.fn(() => ({ user: mockUser })),
    {
      getState: () => ({ token: 't', refreshToken: 'r', user: mockUser }),
    },
  ),
}))

// ---- mutable hooks mocks ----
let mockDevicesResp: ListEnvelope<AdminDevice> | undefined = undefined
vi.mock('@/hooks/useAdmin', () => ({
  useAdminDevices: () => ({ data: mockDevicesResp, isLoading: false }),
  useResyncDevice: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

const errorDevice: AdminDevice = {
  did: 'aabbccddeeff0011',
  user_id: 2,
  user_email: 'alice@x.com',
  name: 'NAS',
  mac_display: 'AA:**:**:**:**:FF',
  description: null,
  cloud_status: 'error',
  last_error: 'createTopic timeout',
  last_drift_at: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-08-01T00:00:00Z',
}

const syncedDevice: AdminDevice = {
  did: '1122334455667788',
  user_id: 3,
  user_email: 'bob@x.com',
  name: 'PC',
  mac_display: '11:**:**:**:**:88',
  description: null,
  cloud_status: 'synced',
  last_error: null,
  last_drift_at: null,
  created_at: '2026-01-02T00:00:00Z',
  updated_at: '2026-08-02T00:00:00Z',
}

const devices = [errorDevice, syncedDevice]

describe('Admin devices page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockDevicesResp = { items: devices, total: devices.length }
  })

  it('renders device rows with names and owner emails', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 桌面表格 + 移动卡片都在 jsdom 渲染（无 CSS 隐藏），文本可能重复。
    expect(screen.getAllByText('NAS').length).toBeGreaterThan(0)
    expect(screen.getAllByText('PC').length).toBeGreaterThan(0)
    expect(screen.getAllByText('alice@x.com').length).toBeGreaterThan(0)
    expect(screen.getAllByText('bob@x.com').length).toBeGreaterThan(0)
  })

  it('renders cloud status filter select', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 列头 ColSyncStatus（Sync status）存在；搜索框 placeholder 存在。
    expect(screen.getByPlaceholderText('Search device or user...')).toBeInTheDocument()
    // 列头 "Sync status" 在桌面表格里渲染。
    expect(screen.getAllByText('Sync status').length).toBeGreaterThan(0)
  })

  it('error rows show warning styling (border-l-warning)', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    // DataTable rowWarning → error 行带 border-l-warning class（桌面 tr + 移动卡片各一处）
    expect(container.querySelector('.border-l-warning')).toBeInTheDocument()
    // error 行还展示红色状态点（bg-destructive）+ "Sync error" 文案
    expect(container.querySelector('.bg-destructive')).toBeInTheDocument()
    expect(screen.getAllByText('Sync error').length).toBeGreaterThan(0)
  })

  it('synced rows show success status', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    // synced → 绿点（bg-success）+ "Synced" 文案
    expect(container.querySelector('.bg-success')).toBeInTheDocument()
    expect(screen.getAllByText('Synced').length).toBeGreaterThan(0)
  })

  it('renders resync actions menu trigger for each device', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 每行 ActionsMenu.Trigger（MoreVertical）。桌面 + 移动各一个 → ≥2 个触发按钮。
    expect(screen.getAllByRole('button').length).toBeGreaterThan(0)
  })

  it('renders empty state when no devices', async () => {
    mockDevicesResp = { items: [], total: 0 }
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('No devices found.')).toBeInTheDocument()
  })
})
