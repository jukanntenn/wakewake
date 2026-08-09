import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import { renderWithProviders } from '@/test/test-utils'

// 不 mock next-intl——renderWithProviders 提供真实的 NextIntlClientProvider

// Mock sonner toast
vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}))

// Mock hooks
const mockCreateIntegration = vi.fn().mockResolvedValue({})
const mockPatchIntegration = vi.fn().mockResolvedValue({})
const mockDeleteIntegration = vi.fn().mockResolvedValue({})
const mockToggleIntegration = vi.fn().mockResolvedValue({})
const mockUseIntegrations = vi.fn(() => ({ data: undefined }))
const mockUseDevices = vi.fn(() => ({ data: [] }))

const baseSchema = {
  properties: {
    uid: { type: 'string', title: 'Bemfa UID', secret: true, description: 'uid desc' },
    secret_id: { type: 'string', title: 'Bemfa secretID', secret: true },
    secret_key: { type: 'string', title: 'Bemfa secretKey', secret: true },
  },
  required: ['uid'],
}

vi.mock('@/hooks/useIntegrations', () => ({
  useIntegrations: (...args: unknown[]) => mockUseIntegrations(...args),
  useIntegrationSchema: () => ({ data: baseSchema }),
  useCreateIntegration: () => ({ mutateAsync: mockCreateIntegration, isPending: false }),
  usePatchIntegration: () => ({ mutateAsync: mockPatchIntegration, isPending: false }),
  useDeleteIntegration: () => ({ mutateAsync: mockDeleteIntegration, isPending: false }),
  useToggleIntegration: () => ({ mutateAsync: mockToggleIntegration, isPending: false }),
}))

vi.mock('@/hooks/useDevices', () => ({
  useDevices: (...args: unknown[]) => mockUseDevices(...args),
}))

vi.mock('@/hooks/useAgents', () => ({
  useDefaultAgent: () => ({
    data: { aid: 'test', public_key: 'PEM', status: 'online', pairing_code: '****' },
  }),
}))

vi.mock('@/lib/crypto', () => ({
  encryptWithPublicKey: vi.fn().mockResolvedValue('encrypted'),
}))

const first = (els: HTMLElement[]) => els[0]

describe('Integrations page (Bemfa)', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseIntegrations.mockReturnValue({ data: undefined })
    mockUseDevices.mockReturnValue({ data: [] })
  })

  it('renders connect form when no integration exists', async () => {
    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    expect(first(screen.getAllByText('Bemfa UID'))).toBeInTheDocument()
    // 无 topic_prefix（已从 schema 移除）
    expect(screen.queryByText('Topic 前缀')).not.toBeInTheDocument()
    // 连接按钮
    expect(screen.getByText('Connect Bemfa')).toBeInTheDocument()
  })

  it('reveals v2 fields on toggle', async () => {
    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    const v2Toggle = screen.getByText('Use v2 API')
    fireEvent.click(v2Toggle)
    expect(first(screen.getAllByText('Bemfa secretID'))).toBeInTheDocument()
    expect(first(screen.getAllByText('Bemfa secretKey'))).toBeInTheDocument()
  })

  it('create flow calls createIntegration (no existing)', async () => {
    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    const connectBtn = screen.getByText('Connect Bemfa')
    fireEvent.click(connectBtn)
    await waitFor(() => expect(mockCreateIntegration).toHaveBeenCalledTimes(1))
    expect(mockPatchIntegration).not.toHaveBeenCalled()
  })

  it('shows summary view (not form) for connected integration', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'connected',
          mqtt_connected: true,
          last_report_at: '2026-01-01T00:00:00Z',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })

    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    // summary 视图：显示"已连接"状态，不显示表单
    expect(screen.getByText('Connected')).toBeInTheDocument()
    expect(screen.queryByText('Bemfa UID')).not.toBeInTheDocument()
    // 有配置/禁用/删除按钮
    expect(screen.getByText('Configure')).toBeInTheDocument()
    expect(screen.getByText('Disable')).toBeInTheDocument()
  })

  it('switches to edit view on configure click', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'connected',
          mqtt_connected: true,
          last_report_at: '2026-01-01T00:00:00Z',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })

    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    fireEvent.click(screen.getByText('Configure'))
    // 编辑模式：显示表单 + 保存/取消
    expect(first(screen.getAllByText('Bemfa UID'))).toBeInTheDocument()
    expect(screen.getByText('Save')).toBeInTheDocument()
    expect(screen.getByText('Cancel')).toBeInTheDocument()
  })

  it('patch flow preserves untouched secrets for existing', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'connected',
          mqtt_connected: true,
          last_report_at: '2026-01-01T00:00:00Z',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })

    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    fireEvent.click(screen.getByText('Configure'))
    // secret 字段为空（不填）→ patch 不提交它（保留 DB 原值）
    fireEvent.click(screen.getByText('Save'))
    await waitFor(() => expect(mockPatchIntegration).toHaveBeenCalledTimes(1))
    const call = mockPatchIntegration.mock.calls[0][0]
    expect(call.data.config).not.toHaveProperty('uid')
    expect(call.data.config).not.toHaveProperty('secret_id')
  })

  it('shows error state with last_error for sync_error integration', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'error',
          mqtt_connected: false,
          last_error: 'uid decrypt failed',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })

    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    expect(screen.getByText('Sync error')).toBeInTheDocument()
    expect(screen.getByText(/Last error: uid decrypt failed/)).toBeInTheDocument()
  })

  it('hides mijia guide when not connected', async () => {
    // 未连接时不显示米家指引
    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    expect(screen.queryByText('How to enable voice control with Xiao Ai')).not.toBeInTheDocument()
  })

  it('shows mijia guide when connected', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'connected',
          mqtt_connected: true,
          last_report_at: '2026-01-01T00:00:00Z',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })
    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    expect(screen.getByText('How to enable voice control with Xiao Ai')).toBeInTheDocument()
  })

  it('shows devices synced count for connected integration with devices', async () => {
    mockUseIntegrations.mockReturnValue({
      data: [
        {
          provider: 'bemfa',
          config: { uid: '***' },
          enabled: true,
          status: 'connected',
          mqtt_connected: true,
          last_report_at: '2026-01-01T00:00:00Z',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
      ],
    })
    mockUseDevices.mockReturnValue({
      data: [{ did: 'a', name: 'PC', mac_display: 'AA:**:**:**:**:FF' }],
    })

    const { default: IntegrationsPage } = await import('./page')
    renderWithProviders(<IntegrationsPage />)
    expect(screen.getByText('1 devices synced')).toBeInTheDocument()
  })
})
