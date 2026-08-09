import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DeviceForm } from './DeviceForm'

// Mock next-intl（返回 key 本身作 fallback）
vi.mock('next-intl', () => ({
  useTranslations: () => (key: string) => key,
}))

// Mock react-query hooks
const mockCreateDevice = vi.fn().mockResolvedValue({})
const mockPatchDevice = vi.fn().mockResolvedValue({})
vi.mock('@/hooks/useDevices', () => ({
  useCreateDevice: () => ({ mutateAsync: mockCreateDevice, isPending: false }),
  usePatchDevice: () => ({ mutateAsync: mockPatchDevice, isPending: false }),
}))

vi.mock('@/hooks/useAgents', () => ({
  useDefaultAgent: () => ({
    data: {
      aid: 'test-aid',
      public_key: 'PEM_PUBLIC_KEY',
      status: 'online',
      pairing_code: 'abcd****',
    },
  }),
}))

// Mock crypto（避免 jsdom 无 Web Crypto）
vi.mock('@/lib/crypto', () => ({
  encryptWithPublicKey: vi.fn().mockResolvedValue('encrypted-mac-base64'),
  maskMACAddress: vi.fn().mockReturnValue('AA:**:**:**:**:FF'),
  isValidMAC: vi.fn().mockReturnValue(true),
  formatMAC: vi.fn().mockImplementation((mac: string) => mac.toUpperCase()),
}))

// Helper：取第一个匹配（避免 StrictMode 双渲染导致 multiple）
const first = (els: HTMLElement[]) => els[0]

describe('DeviceForm', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders name, mac, description fields', () => {
    render(<DeviceForm onDone={vi.fn()} />)
    expect(first(screen.getAllByPlaceholderText('namePlaceholder'))).toBeInTheDocument()
    expect(first(screen.getAllByPlaceholderText('macAddressPlaceholder'))).toBeInTheDocument()
    expect(first(screen.getAllByPlaceholderText('descriptionPlaceholder'))).toBeInTheDocument()
  })

  it('shows create button in create mode', () => {
    render(<DeviceForm onDone={vi.fn()} />)
    expect(screen.getAllByRole('button', { name: 'create' }).length).toBeGreaterThan(0)
  })

  it('calls onDone after successful create', async () => {
    const user = userEvent.setup()
    const onDone = vi.fn()
    render(<DeviceForm onDone={onDone} />)

    await user.type(first(screen.getAllByPlaceholderText('namePlaceholder')), 'My NAS')
    await user.type(
      first(screen.getAllByPlaceholderText('macAddressPlaceholder')),
      'AA:BB:CC:DD:EE:FF',
    )
    await user.click(first(screen.getAllByRole('button', { name: 'create' })))

    expect(mockCreateDevice).toHaveBeenCalled()
    expect(onDone).toHaveBeenCalled()
  })

  it('shows save button in edit mode', () => {
    const device = {
      did: 'abc123',
      name: 'Existing Device',
      mac_display: 'AA:**:**:**:**:FF',
      description: 'old desc',
      agent_online: true,
      projection_status: 'synced' as const,
      cloud_status: 'synced' as const,
      cloud_observed_name: 'Existing Device',
      cloud_observed_at: '2026-01-01T00:00:00Z',
      last_drift_at: null,
      last_drift_kind: null,
      last_error: null,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    }
    render(<DeviceForm device={device} onDone={vi.fn()} />)
    expect(screen.getAllByRole('button', { name: 'save' }).length).toBeGreaterThan(0)
    expect(first(screen.getAllByDisplayValue('Existing Device'))).toBeInTheDocument()
  })

  it('disables mac input in edit mode', () => {
    const device = {
      did: 'abc123',
      name: 'Test',
      mac_display: 'AA:**:**:**:**:FF',
      description: null,
      agent_online: true,
      projection_status: 'synced' as const,
      cloud_status: 'synced' as const,
      cloud_observed_name: 'Test',
      cloud_observed_at: '2026-01-01T00:00:00Z',
      last_drift_at: null,
      last_drift_kind: null,
      last_error: null,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    }
    render(<DeviceForm device={device} onDone={vi.fn()} />)
    const macInput = first(screen.getAllByPlaceholderText('macAddressPlaceholder'))
    expect(macInput).toBeDisabled()
  })
})
