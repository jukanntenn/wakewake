'use client'

// Admin Devices 页（ui-ux-risk-control §9.6）。
// 调查型列表：DataTable + 搜索 + Cloud Status 过滤 + offset 分页。
// 操作：Resync（L1，调用 useResyncDevice(did)）。error 行左侧琥珀边。

import { Suspense } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, MoreVertical } from 'lucide-react'
import { ApiError, type AdminDevice } from '@/lib/api'
import { useAdminDevices, useResyncDevice } from '@/hooks/useAdmin'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Menu } from '@/components/ui/menu'
import { DataTable, type Column } from '@/components/ui/data-table'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { RelativeTime } from '@/components/ui/relative-time'

function AdminDevicesPageInner() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const router = useRouter()
  const searchParams = useSearchParams()

  // URL query 同步（§11.B.4）：搜索词、过滤、页码同步到 URL。
  const q = searchParams.get('q') ?? ''
  const cloudFilter = searchParams.get('cloud_status') ?? 'all'
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? '10')

  const cloud_status =
    cloudFilter === 'all' ? undefined : (cloudFilter as AdminDevice['cloud_status'])
  const { data: resp, isLoading } = useAdminDevices({ cloud_status, q, page, page_size: pageSize })
  const devices = resp?.items ?? []
  const total = resp?.total ?? 0

  const resyncMut = useResyncDevice()

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/devices?${params.toString()}`)
  }

  const onResync = async (d: AdminDevice) => {
    try {
      await resyncMut.mutateAsync(d.did)
      toast.success(t('resyncSuccess'))
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  // Cloud Status 简化徽标（§9.6）：synced=绿；syncing=琥珀脉冲；not_observed=琥珀；error=红；no_integration=灰。
  const cloudStatusCell = (status: AdminDevice['cloud_status']) => {
    const map: Record<
      AdminDevice['cloud_status'],
      {
        color: 'success' | 'active' | 'warning' | 'error' | 'neutral'
        pulse?: boolean
        label: string
      }
    > = {
      synced: { color: 'success', label: t('sync_synced') },
      syncing: { color: 'active', pulse: true, label: t('sync_syncing') },
      not_observed: { color: 'warning', label: 'Not observed' },
      error: { color: 'error', label: t('sync_sync_error') },
      no_integration: { color: 'neutral', label: 'No integration' },
    }
    const cfg = map[status]
    return (
      <span className="inline-flex items-center gap-1.5 text-sm">
        <StatusIndicator color={cfg.color} pulse={cfg.pulse} aria-label={cfg.label} />
        <span className="text-ink-muted">{cfg.label}</span>
      </span>
    )
  }

  const columns: Column<AdminDevice>[] = [
    { key: 'name', label: t('colDevice') },
    { key: 'user_email', label: t('colOwner') },
    { key: 'cloud_status', label: t('colSyncStatus') },
    { key: 'updated_at', label: t('colUpdated') },
    { key: 'actions', label: t('actions' as never) ?? '' },
  ]

  const renderCell = (d: AdminDevice, key: string) => {
    if (key === 'name') {
      return (
        <div>
          <div className="text-ink font-medium">{d.name}</div>
          <div className="text-ink-subtle text-xs">{d.did.slice(0, 12)}…</div>
        </div>
      )
    }
    if (key === 'user_email') {
      return <span className="text-ink-muted text-sm">{d.user_email}</span>
    }
    if (key === 'cloud_status') {
      return cloudStatusCell(d.cloud_status)
    }
    if (key === 'updated_at') {
      return (
        <span className="text-ink-muted text-sm">
          <RelativeTime date={d.updated_at} />
        </span>
      )
    }
    if (key === 'actions') {
      return <DevicesActionsMenu device={d} onResync={onResync} pending={resyncMut.isPending} />
    }
    return null
  }

  const hasActiveFilters = q !== '' || cloudFilter !== 'all'

  return (
    <div className="space-y-4">
      {/* 面包屑 */}
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('devices')}</h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchDevices' as never) ?? 'Search device or user...'}
          defaultValue={q}
          onChange={(e) => updateQuery({ q: e.target.value, page: 1 })}
        />
        <Select
          value={cloudFilter}
          onValueChange={(v) => updateQuery({ cloud_status: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="synced">{t('filterSynced' as never) ?? 'Synced'}</SelectItem>
            <SelectItem value="syncing">{t('filterSyncing' as never) ?? 'Syncing'}</SelectItem>
            <SelectItem value="not_observed">Not observed</SelectItem>
            <SelectItem value="error">Error</SelectItem>
            <SelectItem value="no_integration">No integration</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/devices')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={devices}
        rowKey={(d) => d.did}
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">
            {t('emptyDevices' as never) ?? 'No devices'}
          </p>
        }
        rowWarning={(d) => d.cloud_status === 'error'}
        pagination={{
          page,
          pageSize,
          total,
          onPageChange: (p) => updateQuery({ page: p }),
          onPageSizeChange: (s) => updateQuery({ pageSize: s, page: 1 }),
        }}
        mobile={{
          primary: (d) => <span className="text-ink font-medium">{d.name}</span>,
          secondary: [
            { key: 'user_email', label: t('colOwner') },
            { key: 'cloud_status', label: t('colSyncStatus') },
            { key: 'updated_at', label: t('colUpdated') },
          ],
          actions: (d) => (
            <DevicesActionsMenu device={d} onResync={onResync} pending={resyncMut.isPending} />
          ),
        }}
      />
    </div>
  )
}

function DevicesActionsMenu({
  device,
  onResync,
  pending,
}: {
  device: AdminDevice
  onResync: (d: AdminDevice) => void
  pending: boolean
}) {
  const t = useTranslations('admin')
  return (
    <Menu>
      <Menu.Trigger className="text-ink-muted hover:bg-surface-2 rounded-md p-1">
        <MoreVertical className="h-4 w-4" />
      </Menu.Trigger>
      <Menu.Popup>
        <Menu.Item disabled={pending} onClick={() => onResync(device)}>
          {t('resync')}
        </Menu.Item>
      </Menu.Popup>
    </Menu>
  )
}

// Next.js 16：useSearchParams 需 Suspense 边界（静态导出场景）。
export default function AdminDevicesPage() {
  return (
    <Suspense fallback={null}>
      <AdminDevicesPageInner />
    </Suspense>
  )
}
