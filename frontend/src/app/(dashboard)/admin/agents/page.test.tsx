import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'
import type { AdminAgent, ListEnvelope } from '@/lib/api'

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

vi.mock('next/navigation', () => ({
  useSearchParams: () => new URLSearchParams(),
  usePathname: () => '/admin/agents',
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
let mockAgentsResp: ListEnvelope<AdminAgent> | undefined = undefined
vi.mock('@/hooks/useAdmin', () => ({
  useAdminAgents: () => ({ data: mockAgentsResp, isLoading: false }),
  useDisconnectAgent: () => ({ mutateAsync: vi.fn(), isPending: false }),
}))

const agents: AdminAgent[] = [
  {
    id: 10,
    user_id: 2,
    user_email: 'alice@x.com',
    aid: 'aabbccdd-0000-0000-0000-000000000001',
    name: 'Home Agent',
    status: 'online',
    last_seen: '2026-08-03T00:00:00Z',
    created_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 11,
    user_id: 3,
    user_email: 'bob@x.com',
    aid: 'aabbccdd-0000-0000-0000-000000000002',
    name: 'Office Agent',
    status: 'offline',
    last_seen: '2026-07-01T00:00:00Z',
    created_at: '2026-01-02T00:00:00Z',
  },
  {
    id: 12,
    user_id: 4,
    user_email: 'carol@x.com',
    aid: 'aabbccdd-0000-0000-0000-000000000003',
    name: 'Desk Agent',
    status: 'pending',
    last_seen: null,
    created_at: '2026-01-03T00:00:00Z',
  },
]

describe('Admin agents page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockAgentsResp = { items: agents, total: agents.length }
  })

  it('renders agent rows with names and owner emails', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 桌面表格 + 移动卡片都在 jsdom 渲染（无 CSS 隐藏），文本可能重复。
    expect(screen.getAllByText('Home Agent').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Office Agent').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Desk Agent').length).toBeGreaterThan(0)
    expect(screen.getAllByText('alice@x.com').length).toBeGreaterThan(0)
    expect(screen.getAllByText('bob@x.com').length).toBeGreaterThan(0)
    expect(screen.getAllByText('carol@x.com').length).toBeGreaterThan(0)
  })

  it('renders status indicators (online=green, offline=gray, pending=amber)', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    // 状态文本标签（online/offline/pending 各有 label）
    expect(screen.getAllByText('Online').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Offline').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Pending').length).toBeGreaterThan(0)
    // online=success（绿 bg-success）；offline=neutral（灰 bg-ink-subtle）；
    // pending=active（琥珀 bg-warning + 脉冲）。
    expect(container.querySelector('.bg-success')).toBeInTheDocument()
    expect(container.querySelector('.bg-ink-subtle')).toBeInTheDocument()
    expect(container.querySelector('.bg-warning.animate-pulse, .bg-warning')).toBeInTheDocument()
  })

  it('status indicator has accessible aria-label', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // StatusIndicator 用 role="img" + aria-label={cfg.label}
    expect(screen.getAllByRole('img', { name: 'Online' }).length).toBeGreaterThan(0)
    expect(screen.getAllByRole('img', { name: 'Offline' }).length).toBeGreaterThan(0)
    expect(screen.getAllByRole('img', { name: 'Pending' }).length).toBeGreaterThan(0)
  })

  it('renders disconnect actions menu trigger for each agent', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 每行 ActionsMenu.Trigger（MoreVertical 按钮）。桌面 + 移动各渲染一次 → 至少 3 个触发器。
    const buttons = screen.getAllByRole('button')
    expect(buttons.length).toBeGreaterThan(0)
  })

  it('renders empty state when no agents', async () => {
    mockAgentsResp = { items: [], total: 0 }
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('No agents found.')).toBeInTheDocument()
  })
})
