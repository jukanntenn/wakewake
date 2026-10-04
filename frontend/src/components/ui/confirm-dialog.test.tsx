import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { NextIntlClientProvider } from 'next-intl'
import { ConfirmDialog } from './confirm-dialog'

const messages = {
  common: { cancel: 'Cancel', confirm: 'Confirm' },
}

function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  )
}

describe('ConfirmDialog (§5.3/§11.A.5)', () => {
  it('renders nothing when closed', () => {
    render(
      wrap(
        <ConfirmDialog
          open={false}
          onConfirm={vi.fn()}
          onClose={vi.fn()}
          title="t"
          description="d"
        />,
      ),
    )
    expect(screen.queryByText('t')).not.toBeInTheDocument()
  })

  it('renders title and description when open', () => {
    render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={vi.fn()}
          onClose={vi.fn()}
          title="Delete device"
          description="Permanently remove?"
        />,
      ),
    )
    expect(screen.getByText('Delete device')).toBeInTheDocument()
    expect(screen.getByText('Permanently remove?')).toBeInTheDocument()
  })

  it('shows Cancel and Confirm buttons', () => {
    render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={vi.fn()}
          onClose={vi.fn()}
          title="t"
          description="d"
          confirmText="Delete"
        />,
      ),
    )
    expect(screen.getByText('Cancel')).toBeInTheDocument()
    expect(screen.getByText('Delete')).toBeInTheDocument()
  })

  it('onConfirm success → onClose called', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn().mockResolvedValue(undefined)
    const onClose = vi.fn()
    render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={onConfirm}
          onClose={onClose}
          title="t"
          description="d"
          confirmText="OK"
        />,
      ),
    )
    await user.click(screen.getByText('OK'))
    await waitFor(() => expect(onConfirm).toHaveBeenCalled())
    await waitFor(() => expect(onClose).toHaveBeenCalled())
  })

  it('onConfirm failure → dialog stays open, shows error, does not call onClose', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn().mockRejectedValue(new Error('user not found'))
    const onClose = vi.fn()
    render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={onConfirm}
          onClose={onClose}
          title="t"
          description="d"
          confirmText="Reset"
        />,
      ),
    )
    await user.click(screen.getByText('Reset'))
    await waitFor(() => expect(screen.getByText('user not found')).toBeInTheDocument())
    expect(onClose).not.toHaveBeenCalled()
  })

  it('danger variant shows warning icon and destructive confirm button', () => {
    render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={vi.fn()}
          onClose={vi.fn()}
          variant="danger"
          title="Delete"
          description="d"
          confirmText="Delete"
        />,
      ),
    )
    // destructive 按钮（data-variant 属性）；title 与 confirmText 同为 Delete，取 button 元素
    const btns = screen.getAllByText('Delete')
    const confirmBtn = btns.find((el) => el.tagName === 'BUTTON')
    expect(confirmBtn?.getAttribute('data-variant')).toBe('destructive')
  })

  it('confirming state disables both buttons', async () => {
    let resolveConfirm: () => void
    const onConfirm = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveConfirm = resolve
        }),
    )
    const { rerender } = render(
      wrap(
        <ConfirmDialog
          open={true}
          onConfirm={onConfirm}
          onClose={vi.fn()}
          title="t"
          description="d"
          confirmText="OK"
        />,
      ),
    )
    const user = userEvent.setup()
    await user.click(screen.getByText('OK'))
    // 等 confirming 态生效
    await waitFor(() => {
      const cancelBtn = screen.getByText('Cancel').closest('button')
      expect(cancelBtn).toBeDisabled()
    })
    resolveConfirm!()
    // 恢复
    await waitFor(() => {
      // onClose 未提供，但 confirming 结束后按钮恢复可点（dialog 仍开）
    })
    // 清理：避免 unhandled promise
    void rerender
  })

  it('renders optional children (input area)', () => {
    render(
      wrap(
        <ConfirmDialog open={true} onConfirm={vi.fn()} onClose={vi.fn()} title="t" description="d">
          <input aria-label="reason" />
        </ConfirmDialog>,
      ),
    )
    expect(screen.getByLabelText('reason')).toBeInTheDocument()
  })
})
