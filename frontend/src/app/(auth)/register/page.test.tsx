import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

// 维护态由 useQuery('maintenance-status') 驱动——mock react-query 层，
// 页面其余依赖（solvePow/api）只在提交时触达，渲染路径不需要。
let mockMaintenance: { enabled: boolean; mode: string; message: string } | undefined
vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@tanstack/react-query')>()
  return { ...actual, useQuery: () => ({ data: mockMaintenance }) }
})
vi.mock('next/navigation', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

describe('注册页维护态联动（admin-risk-controls WRFC）', () => {
  beforeEach(() => {
    mockMaintenance = undefined
  })

  it('维护关闭：表单可用，无公告', async () => {
    mockMaintenance = { enabled: false, mode: 'registration_disabled', message: '' }
    const { default: RegisterPage } = await import('./page')
    renderWithProviders(<RegisterPage />)
    expect(screen.getByRole('button', { name: 'Sign up' })).toBeEnabled()
    expect(
      screen.queryByText('Registration is temporarily closed. Please try again later.'),
    ).not.toBeInTheDocument()
  })

  it('维护开启（任意档位均拦注册）：禁用表单并显示公告', async () => {
    mockMaintenance = { enabled: true, mode: 'registration_disabled', message: '' }
    const { default: RegisterPage } = await import('./page')
    renderWithProviders(<RegisterPage />)
    expect(
      screen.getByText('Registration is temporarily closed. Please try again later.'),
    ).toBeVisible()
    expect(screen.getByRole('button', { name: 'Sign up' })).toBeDisabled()
    expect(screen.getByLabelText('Email')).toBeDisabled()
    expect(screen.getByLabelText('Password')).toBeDisabled()
  })

  it('维护开启且带 message：优先显示 message', async () => {
    mockMaintenance = {
      enabled: true,
      mode: 'readonly',
      message: 'ops notice',
    }
    const { default: RegisterPage } = await import('./page')
    renderWithProviders(<RegisterPage />)
    expect(screen.getByText('ops notice')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Sign up' })).toBeDisabled()
  })
})
