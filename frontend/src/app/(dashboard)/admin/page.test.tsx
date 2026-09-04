import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'
import type { AdminStats, RiskOverview } from '@/lib/api'

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

// Mock useAdminStats / useRiskOverview / useAddIpBan via module-level mutables so
// individual tests can set the return value without vi.resetModules/vi.doMock ceremony.
let mockStats: AdminStats | undefined = undefined
let mockIsLoading = false
let mockRisk: RiskOverview | undefined = undefined
const mockBanMutate = vi.fn()
vi.mock('@/hooks/useAdmin', () => ({
  useAdminStats: () => ({ data: mockStats, isLoading: mockIsLoading }),
  useRiskOverview: () => ({ data: mockRisk }),
  useAddIpBan: () => ({ mutateAsync: mockBanMutate, isPending: false }),
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

const healthyRisk: RiskOverview = {
  registrations_24h: 3,
  registrations_7d: 9,
  unverified_count: 2,
  oldest_unverified_age_hours: 30,
  failed_logins_24h: 7,
  top_failed_ips: [
    { ip: '203.0.113.50', failures: 5, distinct_emails: 2 },
    { ip: '198.51.100.9', failures: 2, distinct_emails: 1 },
  ],
  top_failed_emails: [{ email: 'victim@x.com', failures: 6, distinct_ips: 2 }],
  mailer: {
    enabled: true,
    limits: { register: 500, resend: 200, reset: 300 },
    day: '2026-09-04',
    sent: { register: 3, resend: 0, reset: 1 },
    blocked: { register: 0, resend: 0, reset: 0 },
  },
  pow_difficulty: 4,
  ip_ban_count: 1,
  rate_limited_since_start: 12,
  rate_limited_uptime_secs: 3600,
}

describe('Admin overview page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockStats = healthyStats
    mockIsLoading = false
    mockRisk = healthyRisk
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
    expect(screen.getByRole('link', { name: /Email & registration controls/ })).toHaveAttribute(
      'href',
      '/admin/mailer',
    )
    expect(screen.getByRole('link', { name: /IP bans/ })).toHaveAttribute('href', '/admin/ip-bans')
  })

  it('risk section shows signals and top failed IPs with ban buttons', async () => {
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    // 信号数字
    expect(screen.getByText('Sign-ups (24h):')).toBeInTheDocument()
    // top 失败 IP 行 + 封禁按钮
    expect(screen.getByText('203.0.113.50')).toBeInTheDocument()
    const banButtons = screen.getAllByRole('button', { name: 'Ban' })
    expect(banButtons).toHaveLength(2)
  })

  it('clicking ban calls the API with 24h ttl', async () => {
    const user = userEvent.setup()
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    await user.click(screen.getAllByRole('button', { name: 'Ban' })[0])
    expect(mockBanMutate).toHaveBeenCalledWith({ target: '203.0.113.50', ttl_hours: 24 })
  })

  it('healthy state shows success indicator and stats counts', async () => {
    const { default: AdminPage } = await import('./page')
    renderWithProviders(<AdminPage />)
    expect(screen.getByText('All systems operational')).toBeInTheDocument()
    // 用户/活跃计数（语义锚定，避免与风控区块数字碰撞）
    expect(screen.getByText(/Total users/)).toBeInTheDocument()
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
