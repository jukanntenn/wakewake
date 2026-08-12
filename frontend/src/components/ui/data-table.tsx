'use client'

// DataTable：表格封装（§4.6/§13.6/§12.10）。
// 支持可选分页（offset），移动端卡片化（同组件内 md 断点切换）。
// 分页器省略号算法 getPageNumbers（§11.B.5）：总页数≤7 全显，否则首末±当前±省略号。

import * as React from 'react'
import { useTranslations } from 'next-intl'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import { LoadingState } from '@/components/ui/loading-state'

export interface Column<T> {
  key: string
  label: string
  render?: (row: T) => React.ReactNode
  className?: string
}

export interface Pagination {
  page: number
  pageSize: number
  total: number
  onPageChange: (page: number) => void
  onPageSizeChange?: (size: number) => void
}

export interface DataTableMobile<T> {
  primary: (row: T) => React.ReactNode
  secondary: { key: string; label: string }[]
  actions?: (row: T) => React.ReactNode
}

export interface DataTableProps<T> {
  columns: Column<T>[]
  rows: T[]
  rowKey: (row: T) => string | number
  render?: (row: T, key: string) => React.ReactNode
  loading?: boolean
  empty?: React.ReactNode
  pagination?: Pagination
  mobile?: DataTableMobile<T>
  rowWarning?: (row: T) => boolean
  pageSizeOptions?: number[]
}

// §11.B.5 分页省略号算法：总页数≤7 全显；否则首末各 1 + 当前页±1 + 省略号。
function getPageNumbers(page: number, totalPages: number): (number | '…')[] {
  if (totalPages <= 7) return Array.from({ length: totalPages }, (_, i) => i + 1)
  const pages: (number | '…')[] = [1]
  if (page > 3) pages.push('…')
  for (let i = Math.max(2, page - 1); i <= Math.min(totalPages - 1, page + 1); i++) pages.push(i)
  if (page < totalPages - 2) pages.push('…')
  pages.push(totalPages)
  return pages
}

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  render,
  loading,
  empty,
  pagination,
  mobile,
  rowWarning,
  pageSizeOptions = [10, 20, 50],
}: DataTableProps<T>) {
  const t = useTranslations('admin')
  const showSkeleton = loading && rows.length === 0

  if (showSkeleton) {
    return <LoadingState variant="rows" />
  }

  if (rows.length === 0 && empty) {
    return <>{empty}</>
  }

  const cellContent = (row: T, col: Column<T>) =>
    render ? render(row, col.key) : col.render ? col.render(row) : undefined

  return (
    <div className="space-y-4">
      {/* 桌面端表格（md+） */}
      <div className="border-hairline bg-surface-1 hidden overflow-hidden rounded-lg border md:block">
        <table className="w-full">
          <thead>
            <tr className="border-hairline border-b">
              {columns.map((col) => (
                <th
                  key={col.key}
                  scope="col"
                  className="text-ink-muted px-4 py-3 text-left text-xs font-medium tracking-wider uppercase"
                >
                  {col.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, i) => (
              <tr
                key={rowKey(row)}
                className={cn(
                  'border-hairline text-sm transition',
                  i !== rows.length - 1 && 'border-b',
                  'hover:bg-surface-2',
                  rowWarning?.(row) && 'border-l-warning border-l-2',
                )}
              >
                {columns.map((col) => (
                  <td key={col.key} className={cn('text-ink px-4 py-3', col.className)}>
                    {cellContent(row, col)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* 移动端卡片化（<md） */}
      {mobile && (
        <div className="space-y-3 md:hidden">
          {rows.map((row) => (
            <div
              key={rowKey(row)}
              className={cn(
                'border-hairline bg-surface-1 rounded-lg border p-4',
                rowWarning?.(row) && 'border-l-warning border-l-2',
              )}
            >
              <div className="flex items-start justify-between gap-2">
                <span className="text-ink font-medium">{mobile.primary(row)}</span>
                {mobile.actions && <div className="flex shrink-0 gap-1">{mobile.actions(row)}</div>}
              </div>
              <dl className="mt-2 space-y-1 text-sm">
                {mobile.secondary.map((f) => (
                  <div key={f.key} className="flex justify-between gap-2">
                    <dt className="text-ink-muted">{f.label}</dt>
                    <dd className="text-ink text-right">
                      {render ? render(row, f.key) : undefined}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>
      )}

      {/* 分页器 */}
      {pagination && (
        <PaginationBar pagination={pagination} pageSizeOptions={pageSizeOptions} t={t} />
      )}
    </div>
  )
}

// 分页器（§11.B.5）。桌面端全显页码 + 每页条数下拉；移动端简化 prev/[当前]/总/next。
function PaginationBar({
  pagination,
  pageSizeOptions,
  t,
}: {
  pagination: Pagination
  pageSizeOptions: number[]
  t: ReturnType<typeof useTranslations>
}) {
  const { page, pageSize, total, onPageChange, onPageSizeChange } = pagination
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const from = total === 0 ? 0 : (page - 1) * pageSize + 1
  const to = Math.min(page * pageSize, total)
  const pageNumbers = getPageNumbers(page, totalPages)

  return (
    <div className="text-ink-muted flex items-center justify-between text-sm">
      <span>{t('pagination.showing', { from, to, total })}</span>
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1">
          <button
            disabled={page <= 1}
            onClick={() => onPageChange(page - 1)}
            aria-label={t('pagination.prev')}
            className="hover:bg-surface-2 rounded-md p-1.5 disabled:opacity-30"
          >
            <ChevronLeft className="h-4 w-4" />
          </button>
          <div className="hidden items-center gap-1 md:flex">
            {pageNumbers.map((p, idx) =>
              p === '…' ? (
                <span key={`ellipsis-${idx}`} className="text-ink-subtle px-1">
                  …
                </span>
              ) : (
                <button
                  key={p}
                  onClick={() => onPageChange(p)}
                  className={cn(
                    'rounded px-2 py-0.5',
                    p === page ? 'text-ink font-medium' : 'text-ink-muted hover:bg-surface-2',
                  )}
                >
                  {p}
                </button>
              ),
            )}
          </div>
          <span className="text-ink md:hidden">
            [{page}] / {totalPages}
          </span>
          <button
            disabled={page >= totalPages}
            onClick={() => onPageChange(page + 1)}
            aria-label={t('pagination.next')}
            className="hover:bg-surface-2 rounded-md p-1.5 disabled:opacity-30"
          >
            <ChevronRight className="h-4 w-4" />
          </button>
        </div>
        {onPageSizeChange && (
          <select
            value={pageSize}
            onChange={(e) => onPageSizeChange(Number(e.target.value))}
            className="border-hairline bg-canvas rounded-md border px-1.5 py-1 text-xs"
          >
            {pageSizeOptions.map((s) => (
              <option key={s} value={s}>
                {t('pagination.perPage', { count: s })}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  )
}
