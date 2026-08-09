import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithProviders } from '@/test/test-utils'

// 不 mock next-intl——renderWithProviders 提供真实的 NextIntlClientProvider

// Mock sonner（捕获 toast 调用断言路径）
const toast = { success: vi.fn(), error: vi.fn(), message: vi.fn(), warning: vi.fn() }
vi.mock('sonner', () => ({ toast }))

// Mock @/lib/clipboard（控制 copyText 返回值模拟成功/失败/降级）
const mockCopyText = vi.fn()
vi.mock('@/lib/clipboard', () => ({ copyText: (...args: unknown[]) => mockCopyText(...args) }))

// Mock hooks
const mockRotate = vi.fn()
vi.mock('@/hooks/useAgents', () => ({
  useDefaultAgent: () => ({
    data: {
      aid: 'test-aid',
      name: 'Home Agent',
      status: 'pending', // pending → 完整码（非脱敏）
      pairing_code: 'a1b2c3d4e5f60718',
      public_key: '-----BEGIN PUBLIC KEY-----\nTEST\n-----END PUBLIC KEY-----',
      last_seen: '2026-07-16T00:00:00Z',
      created_at: '2026-01-01T00:00:00Z',
    },
  }),
  useRotatePairingCode: () => ({ mutateAsync: mockRotate, isPending: false }),
}))

const first = (els: HTMLElement[]) => els[0]

describe('AgentStatus (agents page) — copy & rotate', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockCopyText.mockResolvedValue(true)
  })

  it('renders agent name and full pairing code', async () => {
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    expect(first(screen.getAllByText('Home Agent'))).toBeInTheDocument()
    expect(first(screen.getAllByText('a1b2c3d4e5f60718'))).toBeInTheDocument()
  })

  it('copy button: success path → toast.success + copyText called', async () => {
    mockCopyText.mockResolvedValue(true)
    const { default: AgentsPage } = await import('./page')
    const { container } = renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    // 复制按钮（aria-label="Copy"）
    const copyBtn = screen.getByRole('button', { name: 'Copy' })
    await user.click(copyBtn)

    expect(mockCopyText).toHaveBeenCalledWith('a1b2c3d4e5f60718')
    await vi.waitFor(() => {
      expect(toast.success).toHaveBeenCalledWith('Copied')
    })
    // 图标切换为 Check（data-state 或 class 验证由 copied state 控制；此处断言 toast 即足够）
    expect(container).toBeDefined()
  })

  it('copy button: failure path → toast.error (非安全上下文降级也失败)', async () => {
    mockCopyText.mockResolvedValue(false)
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'Copy' }))

    expect(mockCopyText).toHaveBeenCalledWith('a1b2c3d4e5f60718')
    await vi.waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('Copy failed')
    })
    expect(toast.success).not.toHaveBeenCalled()
  })

  it('rotate: success + copy success → toast.success + toast.message(newCode)', async () => {
    mockRotate.mockResolvedValue({ pairing_code: 'newcode1234567890' })
    mockCopyText.mockResolvedValue(true)
    // confirm() 在 jsdom 默认返回 true（不弹真实对话框）
    HTMLDialogElement.prototype.showModal = vi.fn()
    window.confirm = vi.fn(() => true)
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'Rotate Code' }))

    // 等异步 mutateAsync 完成
    await vi.waitFor(() => {
      expect(mockRotate).toHaveBeenCalled()
    })
    await vi.waitFor(() => {
      expect(mockCopyText).toHaveBeenCalledWith('newcode1234567890')
    })
    expect(toast.success).toHaveBeenCalledWith('Rotated')
    expect(toast.message).toHaveBeenCalledWith('New code copied to clipboard')
  })

  it('rotate: success + copy fail → toast.warning (不再静默吞错)', async () => {
    mockRotate.mockResolvedValue({ pairing_code: 'newcode1234567890' })
    mockCopyText.mockResolvedValue(false)
    window.confirm = vi.fn(() => true)
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'Rotate Code' }))

    await vi.waitFor(() => {
      expect(mockCopyText).toHaveBeenCalledWith('newcode1234567890')
    })
    expect(toast.warning).toHaveBeenCalledWith(
      "Couldn't copy automatically. Select and copy manually.",
    )
    // 不应弹 message（那是复制成功的提示）
    expect(toast.message).not.toHaveBeenCalled()
  })

  it('shows setup instructions when code is full (pending)', async () => {
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    // 完整码 → 显示命令块（含 wakewake-agent 命令）
    expect(first(screen.getAllByText(/Run on/))).toBeInTheDocument()
  })
})
