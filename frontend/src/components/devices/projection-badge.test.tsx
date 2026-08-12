import { describe, it, expect, vi } from 'vitest'
import { render } from '@testing-library/react'
import { ProjectionBadge } from './projection-badge'

vi.mock('next-intl', () => ({
  useTranslations: () => (key: string) => key,
}))

describe('ProjectionBadge (§14.2 徽标1)', () => {
  it('offline → neutral gray dot', () => {
    const { container } = render(
      <ProjectionBadge agentOnline={false} projectionStatus="agent_offline" />,
    )
    expect(container.querySelector('.bg-ink-subtle')).toBeInTheDocument()
    expect(container.querySelector('.animate-pulse')).not.toBeInTheDocument()
  })

  it('syncing → amber dot with pulse', () => {
    const { container } = render(<ProjectionBadge agentOnline={true} projectionStatus="syncing" />)
    expect(container.querySelector('.bg-warning')).toBeInTheDocument()
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
  })

  it('synced → green dot, no pulse', () => {
    const { container } = render(<ProjectionBadge agentOnline={true} projectionStatus="synced" />)
    expect(container.querySelector('.bg-success')).toBeInTheDocument()
    expect(container.querySelector('.animate-pulse')).not.toBeInTheDocument()
  })

  it('agent_online false with projection_status synced → still neutral (offline wins)', () => {
    const { container } = render(<ProjectionBadge agentOnline={false} projectionStatus="synced" />)
    expect(container.querySelector('.bg-ink-subtle')).toBeInTheDocument()
    expect(container.querySelector('.bg-success')).not.toBeInTheDocument()
  })
})
