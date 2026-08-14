import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { NextIntlClientProvider } from 'next-intl'
import { DataTable, type Column } from './data-table'

const messages = {
  admin: {
    pagination: {
      showing: 'Showing {from}-{to} of {total}',
      prev: 'Previous',
      next: 'Next',
      perPage: '{count} / page',
    },
  },
}

function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={messages}>
      {ui}
    </NextIntlClientProvider>
  )
}

interface Row {
  id: number
  name: string
  status: string
}

const columns: Column<Row>[] = [
  { key: 'name', label: 'Name' },
  { key: 'status', label: 'Status' },
]

const rows: Row[] = [
  { id: 1, name: 'Alice', status: 'active' },
  { id: 2, name: 'Bob', status: 'disabled' },
  { id: 3, name: 'Carol', status: 'active' },
]

describe('DataTable', () => {
  it('renders rows in table (desktop)', () => {
    render(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          render={(row, key) => <>{String((row as unknown as Record<string, unknown>)[key])}</>}
        />,
      ),
    )
    expect(screen.getByText('Name')).toBeInTheDocument()
    expect(screen.getByText('Status')).toBeInTheDocument()
    expect(screen.getByText('Alice')).toBeInTheDocument()
    expect(screen.getByText('Bob')).toBeInTheDocument()
  })

  it('shows skeleton when loading and no rows', () => {
    const { container } = render(
      wrap(<DataTable columns={columns} rows={[]} rowKey={(r) => r.id} loading={true} />),
    )
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
  })

  it('shows empty prop when no rows and not loading', () => {
    render(
      wrap(
        <DataTable columns={columns} rows={[]} rowKey={(r) => r.id} empty={<div>No data</div>} />,
      ),
    )
    expect(screen.getByText('No data')).toBeInTheDocument()
  })

  it('renders pagination with correct info', () => {
    render(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          pagination={{ page: 1, pageSize: 10, total: 25, onPageChange: vi.fn() }}
        />,
      ),
    )
    // from=1, to=min(page*pageSize, total)=min(10,25)=10
    expect(screen.getByText('Showing 1-10 of 25')).toBeInTheDocument()
    // 页码 1,2,3（totalPages=3 ≤7 全显）
    expect(screen.getByText('2')).toBeInTheDocument()
    expect(screen.getByText('3')).toBeInTheDocument()
  })

  it('prev disabled on first page, next disabled on last page', () => {
    const { rerender } = render(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          pagination={{ page: 1, pageSize: 10, total: 25, onPageChange: vi.fn() }}
        />,
      ),
    )
    const prevBtn = screen.getByLabelText('Previous')
    expect(prevBtn).toBeDisabled()

    rerender(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          pagination={{ page: 3, pageSize: 10, total: 25, onPageChange: vi.fn() }}
        />,
      ),
    )
    const nextBtn = screen.getByLabelText('Next')
    expect(nextBtn).toBeDisabled()
  })

  it('onPageChange called with correct page', async () => {
    const user = userEvent.setup()
    const onPageChange = vi.fn()
    render(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          pagination={{ page: 1, pageSize: 10, total: 25, onPageChange }}
        />,
      ),
    )
    await user.click(screen.getByText('2'))
    expect(onPageChange).toHaveBeenCalledWith(2)
  })

  it('applies rowWarning class to warning rows', () => {
    const { container } = render(
      wrap(
        <DataTable
          columns={columns}
          rows={rows}
          rowKey={(r) => r.id}
          rowWarning={(r) => r.status === 'disabled'}
          render={(row, key) => <>{String((row as unknown as Record<string, unknown>)[key])}</>}
        />,
      ),
    )
    expect(container.querySelector('.border-l-warning')).toBeInTheDocument()
  })
})
