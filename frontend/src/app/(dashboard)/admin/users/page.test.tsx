import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

// Mock auth store：当前管理员 id=1（禁用自己应被 disable）
vi.mock('@/stores/auth', () => ({
  useAuthStore: Object.assign(vi.fn(), {
    getState: () => ({ token: 't', refreshToken: 'r', user: { id: 1, is_superuser: true } }),
  }),
}))

const usersData = {
  items: [
    {
      id: 2,
      email: 'alice@x.com',
      is_active: true,
      disabled_at: null,
      is_superuser: false,
      email_verified: true,
      last_login: null,
      created_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 3,
      email: 'bob@x.com',
      is_active: false,
      disabled_at: '2026-08-01T00:00:00Z',
      is_superuser: false,
      email_verified: false,
      last_login: null,
      created_at: '2026-01-02T00:00:00Z',
    },
  ],
}

describe('Admin users page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.resetModules()
  })

  it('渲染用户表格 + 邮箱', async () => {
    vi.doMock('@/hooks/useAdminUsers', () => ({
      useAdminUsers: () => ({ data: usersData.items, isLoading: false }),
      useDisableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useEnableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const mocks = {
      useResetUserPassword: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useVerifyUserEmail: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }
    vi.doMock('@/hooks/useAdmin', () => mocks)
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('alice@x.com')).toBeInTheDocument()
    expect(screen.getByText('bob@x.com')).toBeInTheDocument()
    // 活跃用户显示 Disable，已禁用用户显示 Enable
    expect(screen.getByText('Disable')).toBeInTheDocument()
    expect(screen.getByText('Enable')).toBeInTheDocument()
  })

  it('点击 Reset Password 打开 Dialog，输入短密码禁用提交', async () => {
    const resetMut = vi.fn().mockResolvedValue(undefined)
    vi.doMock('@/hooks/useAdminUsers', () => ({
      useAdminUsers: () => ({ data: usersData.items, isLoading: false }),
      useDisableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useEnableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    vi.doMock('@/hooks/useAdmin', () => ({
      useResetUserPassword: () => ({ mutateAsync: resetMut, isPending: false }),
      useVerifyUserEmail: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)

    // 点击第一个用户的 Reset Password
    const resetBtns = screen.getAllByText('Reset Password')
    fireEvent.click(resetBtns[0])
    // Dialog 出现（newPassword 标签）
    await waitFor(() => expect(screen.getByText('New password')).toBeInTheDocument())
    // 提交按钮初始 disabled（密码为空 < 8）
    const submitBtn = screen.getAllByText('Reset Password').pop() as HTMLElement
    expect(submitBtn).toBeDisabled()
  })

  it('加载中显示 loading', async () => {
    vi.doMock('@/hooks/useAdminUsers', () => ({
      useAdminUsers: () => ({ data: undefined, isLoading: true }),
      useDisableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useEnableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    vi.doMock('@/hooks/useAdmin', () => ({
      useResetUserPassword: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useVerifyUserEmail: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('已验证邮箱用户 Force verify 按钮 disabled', async () => {
    vi.doMock('@/hooks/useAdminUsers', () => ({
      useAdminUsers: () => ({ data: usersData.items, isLoading: false }),
      useDisableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useEnableUser: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    vi.doMock('@/hooks/useAdmin', () => ({
      useResetUserPassword: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useVerifyUserEmail: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }))
    const { default: Page } = await import('./page')
    renderWithProviders(<Page />)
    // alice (已验证) 的 Force verify disabled，bob (未验证) 的 enabled
    const forceBtns = screen.getAllByText('Force verify')
    expect(forceBtns[0]).toBeDisabled() // alice
    expect(forceBtns[1]).toBeEnabled() // bob
  })
})
