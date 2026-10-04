import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { NextIntlClientProvider } from 'next-intl'
import { AsyncView } from './async-view'

const messages = {
  error: { default: 'Something went wrong', checkConnection: 'check' },
  common: { retry: 'Retry' },
}

function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  )
}

const emptyQuery = {
  data: undefined,
  isLoading: false,
  isFetching: false,
  isError: false,
  error: null,
  refetch: vi.fn().mockResolvedValue(undefined),
}

describe('AsyncView 5-state machine (§6/§11.A.4)', () => {
  it('state: isLoading + no data → LoadingState (page spinner)', () => {
    const { container } = render(
      wrap(
        <AsyncView query={{ ...emptyQuery, isLoading: true }} loadingVariant="page">
          {() => <div>content</div>}
        </AsyncView>,
      ),
    )
    expect(container.querySelector('.animate-spin')).toBeInTheDocument()
    expect(screen.queryByText('content')).not.toBeInTheDocument()
  })

  it('state: isError + no data → persistent ErrorState with retry', () => {
    render(
      wrap(
        <AsyncView query={{ ...emptyQuery, isError: true }} empty={<div>empty</div>}>
          {() => <div>content</div>}
        </AsyncView>,
      ),
    )
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.getByText('Retry')).toBeInTheDocument()
  })

  it('state: success + empty → empty prop', () => {
    render(
      wrap(
        <AsyncView query={{ ...emptyQuery, data: [] }} empty={<div>No devices</div>}>
          {() => <div>content</div>}
        </AsyncView>,
      ),
    )
    expect(screen.getByText('No devices')).toBeInTheDocument()
    expect(screen.queryByText('content')).not.toBeInTheDocument()
  })

  it('state: success + data → children', () => {
    render(
      wrap(
        <AsyncView query={{ ...emptyQuery, data: [{ id: 1 }] }} empty={<div>empty</div>}>
          {() => <div>real content</div>}
        </AsyncView>,
      ),
    )
    expect(screen.getByText('real content')).toBeInTheDocument()
  })

  it('state: isFetching + hasData → children + refreshing top bar', () => {
    const { container } = render(
      wrap(
        <AsyncView query={{ ...emptyQuery, data: [{ id: 1 }], isFetching: true }}>
          {() => <div>real content</div>}
        </AsyncView>,
      ),
    )
    expect(screen.getByText('real content')).toBeInTheDocument()
    // refreshing 细条：bg-success + animate-pulse
    const bar = container.querySelector('.bg-success.animate-pulse')
    expect(bar).toBeInTheDocument()
  })

  it('state: isError + hasData → children retained + stale error bar (not full ErrorState)', () => {
    const { container } = render(
      wrap(
        <AsyncView query={{ ...emptyQuery, data: [{ id: 1 }], isError: true }}>
          {() => <div>stale content</div>}
        </AsyncView>,
      ),
    )
    // 旧数据保留
    expect(screen.getByText('stale content')).toBeInTheDocument()
    // 不显示全屏 ErrorState
    expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument()
    // 显示 stale error 红条
    const bar = container.querySelector('.bg-destructive')
    expect(bar).toBeInTheDocument()
  })
})
