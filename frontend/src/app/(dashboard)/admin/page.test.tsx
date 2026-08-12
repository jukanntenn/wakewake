import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'
import type { AdminStats } from '@/lib/api'

// Mock sonner (overview doesn't toast, but keep consistent with sibling admin tests).
vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

// Mock auth store: current user is an admin (admin pages are superuser-only).
const mockUser = { id: 1, email: 'root@x.com', is_superuser: true }
vi.mock('@/stores/auth', () => ({
  useAuthStore: Object.assign(
    vi.fn(() => ({ user: mockUser })),
    {
      getState: () => ({ token: 't', refreshToken: 'r', user: mockUser }),
    },
  ),
}))

// Mock useAdminStats via a module-level mutable so individual tests can set the
// return value without vi.resetModules/vi.doMock ceremony.
let mockStats: AdminStats | undefined = undefined
let mockIsLoading = false
vi.mock('@/hooks/useAdmin', () => ({
  useAdminStats: () => ({ data: mockStats, isLoading: mockIsLoading }),
}))

const healthyStats: AdminStats = {
  users: 5,
  active_users: 4,
  devices: 3,
  agents: 2,
  integrations: 1,
  wakes: 10,
  devices_syncing: 0,
  devices_sync_error: 0,
  online_agents: 1,
}

describe('Admin overview page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockStats = healthyStats
    mockIsLoading = false
  })

  it('renders the Admin title', async () => {
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByRole('heading', { level: 1, name: 'Admin' })).toBeInTheDocument()
  })

  it('renders management entry links', async () => {
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    // 治理层
    expect(screen.getByRole('link', { name: /User Management/ })).toHaveAttribute(
      'href',
      '/admin/users',
    )
    expect(screen.getByRole('link', { name: /Activity/ })).toHaveAttribute(
      'href',
      '/admin/activity',
    )
    // 运维层
    expect(screen.getByRole('link', { name: /^Devices/ })).toHaveAttribute('href', '/admin/devices')
    expect(screen.getByRole('link', { name: /^Agents/ })).toHaveAttribute('href', '/admin/agents')
    expect(screen.getByRole('link', { name: /Integrations/ })).toHaveAttribute(
      'href',
      '/admin/integrations',
    )
    expect(screen.getByRole('link', { name: /Wakes/ })).toHaveAttribute('href', '/admin/wakes')
    // 系统层
    expect(screen.getByRole('link', { name: /Maintenance/ })).toHaveAttribute(
      'href',
      '/admin/maintenance',
    )
  })

  it('healthy state shows success indicator and stats counts', async () => {
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByText('All systems operational')).toBeInTheDocument()
    // 用户/活跃计数
    expect(screen.getByText(/5/)).toBeInTheDocument()
    expect(screen.getByText(/4 active/)).toBeInTheDocument()
  })

  it('shows warning indicator when devices_sync_error > 0', async () => {
    mockStats = { ...healthyStats, devices_sync_error: 2 }
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    // 告警跳转链接：文本包含 "2 device sync errors"，href 含 cloud_status=error
    const errorLink = screen.getByRole('link', { name: /2 device sync errors/ })
    expect(errorLink).toHaveAttribute('href', '/admin/devices?cloud_status=error')
    // 健康态文案不应出现
    expect(screen.queryByText('All systems operational')).not.toBeInTheDocument()
    // 健康仪表盘存在一个 warning 色的 StatusIndicator（圆点 bg-warning）
    expect(document.querySelector('.bg-warning')).toBeInTheDocument()
  })

  it('shows skeleton while loading', async () => {
    mockStats = undefined
    mockIsLoading = true
    const { default: AdminPage } = await import('./page')
    const { container } = renderWithProviders(<AdminPage />)
    expect(screen.getByRole('heading', { level: 1, name: 'Admin' })).toBeInTheDocument()
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
  })
})
