import { describe, it, expect, vi } from 'vitest'
import { render } from '@testing-library/react'
import { CloudBadge, isDriftActive } from './cloud-badge'
import type { Device } from '@/lib/api'

vi.mock('next-intl', () => ({
  useTranslations: () => (key: string, opts?: Record<string, unknown>) =>
    opts && 'error' in opts ? `${key}:${String(opts.error)}` : key,
}))

type DevicePick = Pick<
  Device,
  'agent_online' | 'cloud_status' | 'last_drift_at' | 'last_drift_kind' | 'last_error'
>

const base: DevicePick = {
  agent_online: true,
  cloud_status: 'synced',
  last_drift_at: null,
  last_drift_kind: null,
  last_error: null,
}

describe('isDriftActive', () => {
  it('returns false for null', () => {
    expect(isDriftActive(null)).toBe(false)
  })
  it('returns false for drift older than 24h', () => {
    const old = new Date(Date.now() - 25 * 60 * 60 * 1000).toISOString()
    expect(isDriftActive(old)).toBe(false)
  })
  it('returns true for drift within 24h', () => {
    const recent = new Date(Date.now() - 60 * 1000).toISOString()
    expect(isDriftActive(recent)).toBe(true)
  })
})

describe('CloudBadge priority table (§14.2 徽标2)', () => {
  it('row 1: agent offline → neutral gray cloud', () => {
    const { container } = render(<CloudBadge device={{ ...base, agent_online: false }} />)
    // 离线用 CloudOff 图标 + ink-subtle 色
    expect(container.querySelector('.text-ink-subtle')).toBeInTheDocument()
  })

  it('row 2: no_integration → dashed gray cloud', () => {
    const { container } = render(
      <CloudBadge device={{ ...base, cloud_status: 'no_integration' }} />,
    )
    expect(container.querySelector('.border-dashed')).toBeInTheDocument()
  })

  it('row 3: not_observed → amber cloud (active)', () => {
    const { container } = render(<CloudBadge device={{ ...base, cloud_status: 'not_observed' }} />)
    expect(container.querySelector('.text-warning')).toBeInTheDocument()
  })

  it('row 4: syncing → amber cloud with pulse', () => {
    const { container } = render(<CloudBadge device={{ ...base, cloud_status: 'syncing' }} />)
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
  })

  it('row 5: synced no drift → green cloud', () => {
    const { container } = render(<CloudBadge device={{ ...base, cloud_status: 'synced' }} />)
    expect(container.querySelector('.text-success')).toBeInTheDocument()
    // 无漂移小标
    expect(container.querySelector('.bg-warning')).not.toBeInTheDocument()
  })

  it('row 6: synced + drift within 24h → green cloud + amber sub-badge', () => {
    const recent = new Date(Date.now() - 60 * 1000).toISOString()
    const { container } = render(
      <CloudBadge
        device={{
          ...base,
          cloud_status: 'synced',
          last_drift_at: recent,
          last_drift_kind: 'deleted',
        }}
      />,
    )
    expect(container.querySelector('.text-success')).toBeInTheDocument()
    expect(container.querySelector('.bg-warning')).toBeInTheDocument()
  })

  it('row 6: drift older than 24h → no amber sub-badge (falls back to row 5)', () => {
    const old = new Date(Date.now() - 25 * 60 * 60 * 1000).toISOString()
    const { container } = render(
      <CloudBadge
        device={{ ...base, cloud_status: 'synced', last_drift_at: old, last_drift_kind: 'deleted' }}
      />,
    )
    expect(container.querySelector('.bg-warning')).not.toBeInTheDocument()
  })

  it('row 7: error → red cloud', () => {
    const { container } = render(
      <CloudBadge device={{ ...base, cloud_status: 'error', last_error: 'topic not found' }} />,
    )
    expect(container.querySelector('.text-destructive')).toBeInTheDocument()
  })

  it('priority: agent offline overrides cloud_status=error (row 1 wins over row 7)', () => {
    const { container } = render(
      <CloudBadge device={{ ...base, agent_online: false, cloud_status: 'error' }} />,
    )
    // 离线优先：显示灰 CloudOff，不显示红
    expect(container.querySelector('.text-destructive')).not.toBeInTheDocument()
    expect(container.querySelector('.text-ink-subtle')).toBeInTheDocument()
  })
})
