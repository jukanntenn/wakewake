import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { NextIntlClientProvider } from 'next-intl'
import { DeviceList } from './DeviceList'
import type { Device } from '@/lib/api'

const messages = {
  device: {
    syncing: 'Syncing',
    syncError: 'Sync error',
    agentOnline: 'Agent online',
    agentOffline: 'Agent offline',
    cloudSynced: 'Bemfa synced',
    cloudError: 'Bemfa error: {error}',
    cloudNone: 'Bemfa none',
    cloudNotObserved: 'Waiting',
    cloudSyncing: 'Syncing cloud',
    driftDeleted: 'drift del',
    driftRenamed: 'drift ren',
    wake: 'Wake',
    waking: 'Waking...',
    wakeSuccess: 'ok',
    wakeError: 'err',
    wakeTimeout: 'timeout',
    wakeFailed: 'failed: {message}',
    edit: 'Edit',
    delete: 'Delete',
    cancel: 'Cancel',
    deleteSuccess: 'deleted',
    deleteDialogTitle: 'Delete device',
    deleteDialogDesc: 'Permanently remove "{name}"?',
    addDevice: 'Add device',
    details: 'Device details',
    detailIdentity: 'IDENTITY',
    detailConnection: 'CONNECTION',
    detailCloudSync: 'CLOUD SYNC',
    colMac: 'MAC Address',
    colDescription: 'Description',
    colCreated: 'Created',
    colAgent: 'Agent',
    colLastSeen: 'Last seen',
    colStatus: 'Status',
    colLastSync: 'Last sync',
    colLastDrift: 'Last drift',
    masked: '(masked)',
    noDescription: '—',
    never: '—',
    noDrift: '— (none)',
  },
  common: { cancel: 'Cancel', confirm: 'Confirm' },
}

function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  )
}

const mockDeleteDevice = vi.fn().mockResolvedValue({})
const mockWakeDevice = vi.fn().mockResolvedValue('c_test1234')
vi.mock('@/hooks/useDevices', () => ({
  useDeleteDevice: () => ({ mutateAsync: mockDeleteDevice, isPending: false }),
  useWakeDevice: () => ({ mutateAsync: mockWakeDevice, isPending: false }),
  pollCommandStatus: vi.fn().mockResolvedValue({ status: 'completed', success: true }),
}))

const mockDevice: Device = {
  did: 'test-did-123',
  name: 'My NAS',
  mac_display: 'AA:**:**:**:**:FF',
  description: 'Office NAS',
  agent_online: true,
  agent_name: 'Office-Agent',
  agent_last_seen: '2026-01-01T00:00:00Z',
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

describe('DeviceList (重构后)', () => {
  beforeEach(() => vi.clearAllMocks())

  it('renders device cards with name and mac_display', () => {
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />))
    expect(screen.getAllByText('My NAS').length).toBeGreaterThan(0)
    expect(screen.getByText('AA:**:**:**:**:FF')).toBeInTheDocument()
    expect(screen.getByText('Office NAS')).toBeInTheDocument()
  })

  it('renders online device icon (💻) when agent_online', () => {
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />))
    expect(screen.getByText('💻')).toBeInTheDocument()
  })

  it('renders sleeping device (💤) + opacity-60 when agent offline', () => {
    const offline: Device = {
      ...mockDevice,
      agent_online: false,
      projection_status: 'agent_offline',
    }
    const { container } = render(wrap(<DeviceList devices={[offline]} onEdit={vi.fn()} />))
    expect(screen.getByText('💤')).toBeInTheDocument()
    expect(container.querySelector('.opacity-60')).toBeInTheDocument()
  })

  it('wake button disabled when agent offline', () => {
    const offline: Device = {
      ...mockDevice,
      agent_online: false,
      projection_status: 'agent_offline',
    }
    render(wrap(<DeviceList devices={[offline]} onEdit={vi.fn()} />))
    const wakeBtn = screen.getAllByText('Wake')[0].closest('button')
    expect(wakeBtn).toBeDisabled()
  })

  it('wake button enabled and triggers wake when agent online', async () => {
    const user = userEvent.setup()
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />))
    const wakeBtn = screen.getAllByText('Wake')[0].closest('button')!
    expect(wakeBtn).not.toBeDisabled()
    await user.click(wakeBtn)
    expect(mockWakeDevice).toHaveBeenCalledWith('test-did-123')
  })

  it('calls onEdit when edit button clicked', async () => {
    const user = userEvent.setup()
    const onEdit = vi.fn()
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={onEdit} />))
    await user.click(screen.getByLabelText('Edit'))
    expect(onEdit).toHaveBeenCalledWith(mockDevice)
  })

  it('opens L3 ConfirmDialog when delete clicked, deletes on confirm', async () => {
    const user = userEvent.setup()
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />))
    await user.click(screen.getByLabelText('Delete'))
    // ConfirmDialog 打开
    expect(screen.getByText('Delete device')).toBeInTheDocument()
    // 确认删除
    const confirmBtn = screen.getAllByText('Delete').find((el) => el.tagName === 'BUTTON')!
    await user.click(confirmBtn)
    await waitFor(() => expect(mockDeleteDevice).toHaveBeenCalledWith('test-did-123'))
  })

  it('opens §3.7 detail dialog on ··· click showing three zones + masked MAC', async () => {
    const user = userEvent.setup()
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={vi.fn()} />))
    await user.click(screen.getByLabelText('Device details'))
    // 三区标题
    expect(screen.getByText('IDENTITY')).toBeInTheDocument()
    expect(screen.getByText('CONNECTION')).toBeInTheDocument()
    expect(screen.getByText('CLOUD SYNC')).toBeInTheDocument()
    // MAC 只读 + (masked) 标注
    expect(screen.getByText(/\(masked\)/)).toBeInTheDocument()
    // CONNECTION：agent 名
    expect(screen.getByText('Office-Agent')).toBeInTheDocument()
  })

  it('detail dialog Edit → calls onEdit and closes detail', async () => {
    const user = userEvent.setup()
    const onEdit = vi.fn()
    render(wrap(<DeviceList devices={[mockDevice]} onEdit={onEdit} />))
    await user.click(screen.getByLabelText('Device details'))
    // 详情面板内 Edit 入口（双入口，§3.7）
    const detailEdit = screen
      .getAllByRole('button', { name: /Edit/ })
      .find((b) => b.textContent?.includes('Edit'))
    await user.click(detailEdit!)
    expect(onEdit).toHaveBeenCalledWith(mockDevice)
  })

  it('cloud_status=error → border-l-warning (alert state)', () => {
    const errDevice: Device = {
      ...mockDevice,
      cloud_status: 'error',
      last_error: 'topic not found',
    }
    const { container } = render(wrap(<DeviceList devices={[errDevice]} onEdit={vi.fn()} />))
    expect(container.querySelector('.border-warning')).toBeInTheDocument()
  })

  it('renders multiple devices', () => {
    const devices: Device[] = [
      mockDevice,
      { ...mockDevice, did: 'did2', name: 'Gaming PC', mac_display: 'BB:**:**:**:**:FF' },
    ]
    render(wrap(<DeviceList devices={devices} onEdit={vi.fn()} />))
    expect(screen.getAllByText('My NAS').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Gaming PC').length).toBeGreaterThan(0)
  })
})
