import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DeviceList } from './DeviceList'
import type { Device } from '@/lib/api'

// Mock next-intl：返回 key 本身（便于断言 title 文案）
vi.mock('next-intl', () => ({
  useTranslations: () => (key: string) => key,
}))

// Mock hooks
const mockDeleteDevice = vi.fn().mockResolvedValue({})
const mockWakeDevice = vi.fn().mockResolvedValue('c_test1234')
vi.mock('@/hooks/useDevices', () => ({
  useDeleteDevice: () => ({ mutateAsync: mockDeleteDevice, isPending: false }),
  useWakeDevice: () => ({ mutateAsync: mockWakeDevice, isPending: false }),
  // pollCommandStatus：立即返回 completed success（避免轮询循环）
  pollCommandStatus: vi.fn().mockResolvedValue({ status: 'completed', success: true }),
}))

// device-sync-v3 v3 Device 形状（agent_online + projection_status + cloud_status）
const mockDevice: Device = {
  did: 'test-did-123',
  name: 'My NAS',
  mac_display: 'AA:**:**:**:**:FF',
  description: 'Office NAS',
  agent_online: true,
  projection_status: 'synced',
  cloud_status: 'synced',
  cloud_observed_name: 'My NAS',
  cloud_observed_at: '2026-01-01T00:00:00Z',
  last_drift_at: null,
  last_drift_kind: null,
  last_error: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

describe('DeviceList', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders empty state when no devices', () => {
    render(<DeviceList devices={[]} onEdit={vi.fn()} />)
    expect(screen.getByText('noDevices')).toBeInTheDocument()
  })

  it('renders device cards with name and mac_display', () => {
    render(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />)
    expect(screen.getAllByText('My NAS').length).toBeGreaterThan(0)
    expect(screen.getByText('AA:**:**:**:**:FF')).toBeInTheDocument()
    expect(screen.getByText('Office NAS')).toBeInTheDocument()
  })

  it('calls onEdit when edit button clicked', async () => {
    const user = userEvent.setup()
    const onEdit = vi.fn()
    render(<DeviceList devices={[mockDevice]} onEdit={onEdit} />)

    const editBtn = screen.getByLabelText('edit')
    await user.click(editBtn)
    expect(onEdit).toHaveBeenCalledWith(mockDevice)
  })

  it('shows wake button and triggers wake flow when agent online', async () => {
    const user = userEvent.setup()
    render(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />)

    const wakeBtn = screen.getAllByText('wake')[0]
    await user.click(wakeBtn)
    // wake mutation 被调用
    expect(mockWakeDevice).toHaveBeenCalledWith('test-did-123')
  })

  it('disables wake button when agent offline', () => {
    const offlineDevice: Device = {
      ...mockDevice,
      agent_online: false,
      projection_status: 'agent_offline',
    }
    render(<DeviceList devices={[offlineDevice]} onEdit={vi.fn()} />)
    const wakeBtn = screen.getAllByText('wake')[0]
    expect(wakeBtn).toBeDisabled()
  })

  it('shows projection badge (synced + online → green, title agentOnline)', () => {
    render(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />)
    // synced device + online agent → 绿点，title 为 agentOnline
    const badge = document.querySelector('[title="agentOnline"]')
    expect(badge).toBeInTheDocument()
  })

  it('shows syncing projection badge (yellow) when projection_status syncing', () => {
    const syncingDevice: Device = { ...mockDevice, projection_status: 'syncing' }
    render(<DeviceList devices={[syncingDevice]} onEdit={vi.fn()} />)
    const badge = document.querySelector('[title="syncing"]')
    expect(badge).toBeInTheDocument()
  })

  it('shows cloud synced icon (green, title cloudSynced)', () => {
    render(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />)
    const cloudIcon = document.querySelector('[title="cloudSynced"]')
    expect(cloudIcon).toBeInTheDocument()
  })

  it('shows cloud none icon (dashed, title cloudNone) when no_integration', () => {
    const noIntegDevice: Device = { ...mockDevice, cloud_status: 'no_integration' }
    const { container } = render(<DeviceList devices={[noIntegDevice]} onEdit={vi.fn()} />)
    expect(document.querySelector('[title="cloudNone"]')).toBeInTheDocument()
    // §14.2：no_integration 徽标用虚线边框表达「未启用/虚位以待」
    expect(container.querySelector('.border-dashed')).toBeInTheDocument()
  })

  it('renders multiple devices', () => {
    const devices: Device[] = [
      mockDevice,
      { ...mockDevice, did: 'did2', name: 'Gaming PC', mac_display: 'BB:**:**:**:**:FF' },
    ]
    render(<DeviceList devices={devices} onEdit={vi.fn()} />)
    expect(screen.getAllByText('My NAS').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Gaming PC').length).toBeGreaterThan(0)
  })
})
