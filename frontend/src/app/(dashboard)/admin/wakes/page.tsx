'use client'

// Admin Wakes 页（ui-ux-risk-control §9.9）。
// 跨用户唤醒审计列表：DataTable + 搜索 + Type/Result 过滤 + offset 分页。
// Type：wol→Manual，bemfa_wake→Voice。Result 单元格使用 WakeStatusBadge（3 态）。

import { Suspense } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { ChevronLeft } from 'lucide-react'
import { type AdminWake } from '@/lib/api'
import { useAdminWakes } from '@/hooks/useAdmin'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { DataTable, type Column } from '@/components/ui/data-table'
import { WakeStatusBadge } from '@/components/wakes/status-badge'
import { RelativeTime } from '@/components/ui/relative-time'

function AdminWakesPageInner() {
  const t = useTranslations('admin')
  const router = useRouter()
  const searchParams = useSearchParams()

  // URL query 同步（§11.B.4）。camelCase URL 键，hook 参数为 snake_case。
  const q = searchParams.get('q') ?? ''
  const wakeTypeFilter = searchParams.get('wake_type') ?? 'all' // all/wol/bemfa_wake
  const resultFilter = searchParams.get('result') ?? 'all' // all/success/failed/expired
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? '10')

  const wake_type = wakeTypeFilter === 'all' ? undefined : wakeTypeFilter
  const result = resultFilter === 'all' ? undefined : resultFilter
  const { data: resp, isLoading } = useAdminWakes({
    q,
    wake_type,
    result,
    page,
    page_size: pageSize,
  })
  const wakes = resp?.items ?? []
  const total = resp?.total ?? 0

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/wakes?${params.toString()}`)
  }

  // Type 单元格：wol→Manual，bemfa_wake→Voice。
  const typeLabel = (type: AdminWake['type']) => (type === 'wol' ? 'Manual' : 'Voice')

  const columns: Column<AdminWake>[] = [
    { key: 'created_at', label: t('colTime') },
    { key: 'device_name', label: t('colDevice' as never) ?? 'Device' },
    { key: 'user_email', label: t('colOwner' as never) ?? 'User' },
    { key: 'type', label: t('colType' as never) ?? 'Type' },
    { key: 'status', label: t('colResult' as never) ?? 'Result' },
  ]

  const renderCell = (w: AdminWake, key: string) => {
    if (key === 'created_at') {
      return (
        <span className="text-ink-muted text-sm whitespace-nowrap">
          <RelativeTime date={w.created_at} />
        </span>
      )
    }
    if (key === 'device_name') {
      return (
        <div>
          <div className="text-ink text-sm font-medium">{w.device_name}</div>
          <div className="text-ink-subtle text-xs">{w.device_did.slice(0, 12)}…</div>
        </div>
      )
    }
    if (key === 'user_email') {
      return <span className="text-ink-muted text-sm">{w.user_email}</span>
    }
    if (key === 'type') {
      return <span className="text-ink-muted text-sm">{typeLabel(w.type)}</span>
    }
    if (key === 'status') {
      return <WakeStatusBadge status={w.status} message={w.message} />
    }
    return null
  }

  const hasActiveFilters = q !== '' || wakeTypeFilter !== 'all' || resultFilter !== 'all'

  return (
    <div className="space-y-4">
      {/* 面包屑 */}
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink text-2xl font-semibold tracking-tight">
        {t('wakes' as never) ?? 'Wakes'}
      </h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchWakes' as never) ?? 'Search device or user...'}
          defaultValue={q}
          onChange={(e) => updateQuery({ q: e.target.value, page: 1 })}
        />
        <Select
          value={wakeTypeFilter}
          onValueChange={(v) => updateQuery({ wake_type: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="wol">Manual</SelectItem>
            <SelectItem value="bemfa_wake">Voice</SelectItem>
          </SelectContent>
        </Select>
        <Select
          value={resultFilter}
          onValueChange={(v) => updateQuery({ result: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="success">Success</SelectItem>
            <SelectItem value="failed">Failed</SelectItem>
            <SelectItem value="expired">Expired</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/wakes')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={wakes}
        rowKey={(w) => w.id}
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">
            {t('emptyWakes' as never) ?? 'No wakes'}
          </p>
        }
        pagination={{
          page,
          pageSize,
          total,
          onPageChange: (p) => updateQuery({ page: p }),
          onPageSizeChange: (s) => updateQuery({ pageSize: s, page: 1 }),
        }}
        mobile={{
          primary: (w) => (
            <span className="text-ink-muted text-sm">
              <RelativeTime date={w.created_at} />
            </span>
          ),
          secondary: [
            { key: 'device_name', label: t('colDevice' as never) ?? 'Device' },
            { key: 'user_email', label: t('colOwner' as never) ?? 'User' },
            { key: 'type', label: t('colType' as never) ?? 'Type' },
            { key: 'status', label: t('colResult' as never) ?? 'Result' },
          ],
        }}
      />
    </div>
  )
}

// Next.js 16：useSearchParams 需 Suspense 边界（静态导出场景）。
export default function AdminWakesPage() {
  return (
    <Suspense fallback={null}>
      <AdminWakesPageInner />
    </Suspense>
  )
}
