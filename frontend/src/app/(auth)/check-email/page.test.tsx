import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { NextIntlClientProvider } from 'next-intl'
import { ApiError } from '@/lib/api'
import authEn from '@/messages/en.json'

// 与 landing.test.tsx 同款:局部 wrap + 真实 en.json 的 auth 命名空间(防文案漂移)。
function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={{ auth: authEn.auth }}>
      {ui}
    </NextIntlClientProvider>
  )
}

const resendMock = vi.fn()

vi.mock('next/navigation', () => ({
  useSearchParams: () => new URLSearchParams('email=user@example.com'),
}))

vi.mock('@/lib/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/api')>()
  return {
    ApiError: actual.ApiError,
    api: { auth: { resendVerification: (...args: unknown[]) => resendMock(...args) } },
  }
})

import CheckEmailPage from './page'

describe('check-email resend 倒计时(UX 修复)', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resendMock.mockReset()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('挂载即进入 60s 倒计时:按钮禁用并显示剩余时间', () => {
    render(wrap(<CheckEmailPage />))
    const btn = screen.getByRole('button', { name: 'Resend (1:00)' })
    expect(btn).toBeDisabled()
    act(() => {
      vi.advanceTimersByTime(30_000)
    })
    expect(screen.getByRole('button', { name: 'Resend (0:30)' })).toBeDisabled()
  })

  it('倒计时走完:按钮恢复可用', () => {
    render(wrap(<CheckEmailPage />))
    act(() => {
      vi.advanceTimersByTime(60_000)
    })
    expect(screen.getByRole('button', { name: 'Resend email' })).toBeEnabled()
  })

  it('点击成功:重置 60s 倒计时', async () => {
    resendMock.mockResolvedValue({ sent: true })
    render(wrap(<CheckEmailPage />))
    act(() => {
      vi.advanceTimersByTime(60_000)
    })
    const btn = screen.getByRole('button', { name: 'Resend email' })
    await act(async () => {
      btn.click()
    })
    expect(resendMock).toHaveBeenCalledWith('user@example.com')
    expect(screen.getByRole('button', { name: 'Resend (1:00)' })).toBeDisabled()
  })

  it('429 带 Retry-After:倒计时取真实回补窗口(根因回归测试)', async () => {
    resendMock.mockRejectedValue(
      new ApiError('RATE_LIMITED', 'Too many requests', 429, undefined, 1200),
    )
    render(wrap(<CheckEmailPage />))
    act(() => {
      vi.advanceTimersByTime(60_000)
    })
    await act(async () => {
      screen.getByRole('button', { name: 'Resend email' }).click()
    })
    // 旧版 bug:429 真实窗口 20 分钟,文案却固定"等一分钟";现在倒计时如实显示 20:00。
    expect(screen.getByRole('button', { name: 'Resend (20:00)' })).toBeDisabled()
  })

  it('429 无 Retry-After:兜底本地冷却 60s', async () => {
    resendMock.mockRejectedValue(new ApiError('RATE_LIMITED', 'Too many requests', 429))
    render(wrap(<CheckEmailPage />))
    act(() => {
      vi.advanceTimersByTime(60_000)
    })
    await act(async () => {
      screen.getByRole('button', { name: 'Resend email' }).click()
    })
    expect(screen.getByRole('button', { name: 'Resend (1:00)' })).toBeDisabled()
  })
})
