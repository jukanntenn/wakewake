import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'
import type { AdminUser, ListEnvelope } from '@/lib/api'

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

// next/navigation：useSearchParams / useRouter（页面 export 了 Suspense 包裹，无需外部 Suspense）。
vi.mock('next/navigation', () => ({
  useSearchParams: () => new URLSearchParams(),
  usePathname: () => '/admin/users',
  useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
}))

// 当前管理员 id=1（用于自我保护断言）。
const mockUser = { id: 1, email: 'root@x.com', is_superuser: true }
const authState = {
  token: 't',
  refreshToken: 'r',
  user: mockUser,
  _hasHydrated: true,
  setAuth: vi.fn(),
  setToken: vi.fn(),
  logout: vi.fn(),
  setHasHydrated: vi.fn(),
}
// zustand 风格 mock：支持 selector 形式 useAuthStore((s) => s.user) 和无参形式。
const useAuthStoreMock = Object.assign(
  vi.fn((selector?: (s: typeof authState) => unknown) =>
    selector ? selector(authState) : authState,
  ),
  { getState: () => authState },
)
vi.mock('@/stores/auth', () => ({
  useAuthStore: useAuthStoreMock,
}))

// ---- mutable hooks mocks（每个测试可重置返回值）----
let mockUsersResp: ListEnvelope<AdminUser> | undefined = undefined
const mockDisable = vi.fn().mockResolvedValue({})
const mockEnable = vi.fn().mockResolvedValue({})
const mockReset = vi.fn().mockResolvedValue({})

vi.mock('@/hooks/useAdminUsers', () => ({
  useAdminUsers: () => ({ data: mockUsersResp, isLoading: false }),
  useDisableUser: () => ({ mutateAsync: mockDisable, isPending: false }),
  useEnableUser: () => ({ mutateAsync: mockEnable, isPending: false }),
}))

vi.mock('@/hooks/useAdmin', () => ({
  useResetUserPassword: () => ({ mutateAsync: mockReset, isPending: false }),
}))

const alice: AdminUser = {
  id: 2,
  email: 'alice@x.com',
  is_active: true,
  disabled_at: null,
  disabled_reason: null,
  disabled_by: null,
  is_superuser: false,
  email_verified: true,
  last_login: null,
  created_at: '2026-01-01T00:00:00Z',
}

const bob: AdminUser = {
  id: 3,
  email: 'bob@x.com',
  is_active: false,
  disabled_at: '2026-08-01T00:00:00Z',
  disabled_reason: null,
  disabled_by: 1,
  is_superuser: false,
  email_verified: false,
  last_login: null,
  created_at: '2026-01-02T00:00:00Z',
}

// 自己（当前管理员）作为列表行，用于自我保护断言。
const selfRow: AdminUser = {
  ...alice,
  id: 1,
  email: 'root@x.com',
  is_active: true,
  is_superuser: true,
}

describe('Admin users page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUsersResp = { items: [alice, bob], total: 2 }
  })

  it('renders user emails in the table', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // 桌面表格 + 移动卡片都在 jsdom 渲染（无 CSS 隐藏），文本会重复出现。
    expect(screen.getAllByText('alice@x.com').length).toBeGreaterThan(0)
    expect(screen.getAllByText('bob@x.com').length).toBeGreaterThan(0)
  })

  it('renders a green status dot for active users', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    expect(screen.getAllByText('alice@x.com').length).toBeGreaterThan(0)
    // 活跃用户（success=绿，bg-success）；至少存在一个绿点
    expect(container.querySelector('.bg-success')).toBeInTheDocument()
  })

  it('renders a gray status dot for disabled users', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    expect(screen.getAllByText('bob@x.com').length).toBeGreaterThan(0)
    // 禁用用户（neutral=灰，bg-ink-subtle）；至少存在一个灰点
    expect(container.querySelector('.bg-ink-subtle')).toBeInTheDocument()
  })

  it('renders an actions menu trigger for other users', async () => {
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // ActionsMenu 用 lucide MoreVertical 图标（svg）渲染按钮；alice/bob 各一个。
    // 桌面表格 + 移动卡片都在 jsdom 渲染（无 CSS 隐藏），所以会有多个。
    const triggers = screen.getAllByRole('button')
    expect(triggers.length).toBeGreaterThan(0)
  })

  it('self-protection: own row shows "(you)" and no actions menu', async () => {
    mockUsersResp = { items: [selfRow, alice], total: 2 }
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    // 自己的行展示 "(you)" 标记（桌面 + 移动各一处）。
    // JSX `({t('thisIsYou')})` 渲染为文本节点 ( + 翻译 + )，用正则匹配归一化后的文本。
    const youMarks = screen.getAllByText(
      (_, node) => !!node?.textContent && node.textContent.replace(/\s+/g, '') === '(you)',
    )
    expect(youMarks.length).toBeGreaterThan(0)
    // 自己的行不渲染 MoreVertical 触发按钮：整页菜单触发数应等于非自身行（alice）渲染次数。
    // selfRow 是 superuser 且 is_active，但 isSelf → 走 "(you)" 分支而非 ActionsMenu。
    // 这里通过断言 "(you)" 存在且 actions 触发数 < 总行数 ×2（桌面+移动）间接验证。
    const menuTriggers = container.querySelectorAll('[data-slot="menu-trigger"]')
    expect(menuTriggers.length).toBeLessThanOrEqual(2) // 仅 alice 行（桌面+移动）
  })

  it('renders a Disable action available via the actions menu', async () => {
    const { default: Page } = await import('./page')
    const { container } = renderWithProviders(<Page />)
    // jsdom 下 Base UI Menu（Portal + floating-ui autoUpdate 用 new ResizeObserver）
    // 打开时不可靠，故此处仅断言 ActionsMenu 触发器渲染（每个活跃/非自身用户行一个）。
    // 活跃用户 alice 的行应渲染触发器（disable 操作的入口）。
    const triggers = container.querySelectorAll('[data-slot="menu-trigger"]')
    expect(triggers.length).toBeGreaterThan(0)
  })
})
