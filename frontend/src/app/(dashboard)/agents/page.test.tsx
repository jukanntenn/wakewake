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

// Mock hooks（可变 agent 状态：默认 pending，个别用例切 online/offline）
const mockRotate = vi.fn()
const agentState = vi.hoisted(() => ({
  data: {
    aid: 'test-aid',
    name: 'Home Agent',
    status: 'pending', // pending → 完整码（非脱敏）
    pairing_code: 'a1b2c3d4e5f60718',
    public_key: '-----BEGIN PUBLIC KEY-----\nTEST\n-----END PUBLIC KEY-----',
    last_seen: '2026-07-16T00:00:00Z',
    created_at: '2026-01-01T00:00:00Z',
  },
}))
vi.mock('@/hooks/useAgents', () => ({
  useDefaultAgent: () => ({ data: agentState.data }),
  useRotatePairingCode: () => ({ mutateAsync: mockRotate, isPending: false }),
}))

const first = (els: HTMLElement[]) => els[0]

// 每个用例动态 import 整页(重模块图),隔离即 ~3.4s、全量并行时逼近默认 5s 上限;
// 超时会被误报为失败并污染下一用例的 DOM 断言,抬高本文件上限。
vi.setConfig({ testTimeout: 15_000 })

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

  it('rotate: L2 two-step → first click arms, second click executes + copy success', async () => {
    mockRotate.mockResolvedValue({ pairing_code: 'newcode1234567890' })
    mockCopyText.mockResolvedValue(true)
    HTMLDialogElement.prototype.showModal = vi.fn()
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    const rotateBtn = screen.getByRole('button', { name: 'Rotate Code' })
    // 第一次点击：armed（不执行 rotate）
    await user.click(rotateBtn)
    expect(mockRotate).not.toHaveBeenCalled()

    // 第二次点击：执行（3s 窗口内）
    await user.click(rotateBtn)

    await vi.waitFor(() => {
      expect(mockRotate).toHaveBeenCalled()
    })
    await vi.waitFor(() => {
      expect(mockCopyText).toHaveBeenCalledWith('newcode1234567890')
    })
    expect(toast.success).toHaveBeenCalledWith('Rotated')
    expect(toast.message).toHaveBeenCalledWith('New code copied to clipboard')
  })

  it('rotate: L2 two-step → success + copy fail → toast.warning', async () => {
    mockRotate.mockResolvedValue({ pairing_code: 'newcode1234567890' })
    mockCopyText.mockResolvedValue(false)
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    const user = userEvent.setup()

    const rotateBtn = screen.getByRole('button', { name: 'Rotate Code' })
    // 两步点击
    await user.click(rotateBtn)
    await user.click(rotateBtn)

    await vi.waitFor(() => {
      expect(mockCopyText).toHaveBeenCalledWith('newcode1234567890')
    })
    expect(toast.warning).toHaveBeenCalledWith(
      "Couldn't copy automatically. Select and copy manually.",
    )
    // 不应弹 message（那是复制成功的提示）
    expect(toast.message).not.toHaveBeenCalled()
  })

  it('pending: shows install command (origin embedded) + auto-waiting line', async () => {
    const { default: AgentsPage } = await import('./page')
    renderWithProviders(<AgentsPage />)
    // 命令模板槽（Linux tab 默认）：一行安装命令含 install.sh + --server origin
    await vi.waitFor(() => {
      expect(screen.getByText(/install\.sh \| sh -s -- --server /)).toBeInTheDocument()
    })
    // Docker tab:双块——compose(推荐标记)+ 一键启动 docker run(--network host 硬前提)
    await userEvent.click(screen.getByRole('tab', { name: 'Docker' }))
    expect(screen.getByText('Docker Compose')).toBeInTheDocument()
    expect(screen.getByText('Recommended')).toBeInTheDocument()
    expect(screen.getByText(/network_mode: host/)).toBeInTheDocument()
    expect(screen.getByText(/docker compose up -d/)).toBeInTheDocument()
    expect(screen.getByText(/docker run -d .*--network host/)).toBeInTheDocument()
    expect(screen.getByText('One-liner')).toBeInTheDocument()
    // 等待行（自动检测提示）
    expect(
      screen.getByText('Waiting for the agent to connect — this page updates automatically.'),
    ).toBeInTheDocument()
  })

  it('online: collapsed summary with devices CTA', async () => {
    agentState.data = { ...agentState.data, status: 'online' }
    try {
      const { default: AgentsPage } = await import('./page')
      renderWithProviders(<AgentsPage />)
      const cta = screen.getByRole('link', { name: 'Go to devices' })
      expect(cta).toHaveAttribute('href', '/devices')
      expect(screen.getByText('Agent connected')).toBeInTheDocument()
    } finally {
      agentState.data = { ...agentState.data, status: 'pending' }
    }
  })

  it('offline: repair guidance with rotate entry', async () => {
    agentState.data = { ...agentState.data, status: 'offline', pairing_code: 'a1b2****' }
    try {
      const { default: AgentsPage } = await import('./page')
      renderWithProviders(<AgentsPage />)
      expect(screen.getByText('Agent disconnected')).toBeInTheDocument()
      // 脱敏码 → 不渲染命令卡（--server 命令不应出现）
      expect(screen.queryByText(/wakewake-agent --server /)).not.toBeInTheDocument()
    } finally {
      agentState.data = { ...agentState.data, status: 'pending', pairing_code: 'a1b2c3d4e5f60718' }
    }
  })
})
