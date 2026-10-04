'use client'

// Admin Integrations 页（ui-ux-risk-control §9.8）。
// 调查型列表：DataTable + 搜索 + Status 过滤 + offset 分页。
// 操作：Resync（L1，调用 useResyncIntegration(id)）。error 行左侧琥珀边。
// Status 单元格使用 IntegrationStatusBadge（7 态）。

import { Suspense } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, MoreVertical } from 'lucide-react'
import { ApiError, type AdminIntegration } from '@/lib/api'
import { useAdminIntegrations, useResyncIntegration } from '@/hooks/useAdmin'
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
import { IntegrationStatusBadge } from '@/components/integrations/status-badge'
import { RelativeTime } from '@/components/ui/relative-time'

function AdminIntegrationsPageInner() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const router = useRouter()
  const searchParams = useSearchParams()

  // URL query 同步（§11.B.4）。
  const q = searchParams.get('q') ?? ''
  const statusFilter = searchParams.get('status') ?? 'all'
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? '10')

  const status = statusFilter === 'all' ? undefined : statusFilter
  const { data: resp, isLoading } = useAdminIntegrations({ status, q, page, page_size: pageSize })
  const integrations = resp?.items ?? []
  const total = resp?.total ?? 0

  const resyncMut = useResyncIntegration()

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/integrations?${params.toString()}`)
  }

  const onResync = async (it: AdminIntegration) => {
    try {
      await resyncMut.mutateAsync(it.id)
      toast.success(t('resyncSuccess'))
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  const columns: Column<AdminIntegration>[] = [
    { key: 'provider', label: t('colProvider' as never) ?? 'Provider' },
    { key: 'user_email', label: t('colOwner' as never) ?? 'User' },
    { key: 'status', label: t('colStatus') },
    { key: 'last_report_at', label: t('colLastReport' as never) ?? 'Last report' },
    { key: 'actions', label: t('actions' as never) ?? '' },
  ]

  const renderCell = (it: AdminIntegration, key: string) => {
    if (key === 'provider') {
      return <span className="text-ink text-sm font-medium">{it.provider}</span>
    }
    if (key === 'user_email') {
      return <span className="text-ink-muted text-sm">{it.user_email}</span>
    }
    if (key === 'status') {
      return <IntegrationStatusBadge status={it.status} lastError={it.last_error} />
    }
    if (key === 'last_report_at') {
      return (
        <span className="text-ink-muted text-sm">
          <RelativeTime date={it.last_report_at} />
        </span>
      )
    }
    if (key === 'actions') {
      return (
        <IntegrationsActionsMenu
          integration={it}
          onResync={onResync}
          pending={resyncMut.isPending}
        />
      )
    }
    return null
  }

  const hasActiveFilters = q !== '' || statusFilter !== 'all'

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
        {t('integrations' as never) ?? 'Integrations'}
      </h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchIntegrations' as never) ?? 'Search provider or user...'}
          defaultValue={q}
          onChange={(e) => updateQuery({ q: e.target.value, page: 1 })}
        />
        <Select
          value={statusFilter}
          onValueChange={(v) => updateQuery({ status: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="connected">Connected</SelectItem>
            <SelectItem value="connecting">Connecting</SelectItem>
            <SelectItem value="disconnected">Disconnected</SelectItem>
            <SelectItem value="error">Error</SelectItem>
            <SelectItem value="disabled">Disabled</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/integrations')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={integrations}
        rowKey={(it) => it.id}
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">
            {t('emptyIntegrations' as never) ?? 'No integrations'}
          </p>
        }
        rowWarning={(it) => it.status === 'error'}
        pagination={{
          page,
          pageSize,
          total,
          onPageChange: (p) => updateQuery({ page: p }),
          onPageSizeChange: (s) => updateQuery({ pageSize: s, page: 1 }),
        }}
        mobile={{
          primary: (it) => <span className="text-ink font-medium">{it.user_email}</span>,
          secondary: [
            { key: 'provider', label: t('colProvider' as never) ?? 'Provider' },
            { key: 'status', label: t('colStatus') },
          ],
          actions: (it) => (
            <IntegrationsActionsMenu
              integration={it}
              onResync={onResync}
              pending={resyncMut.isPending}
            />
          ),
        }}
      />
    </div>
  )
}

function IntegrationsActionsMenu({
  integration,
  onResync,
  pending,
}: {
  integration: AdminIntegration
  onResync: (it: AdminIntegration) => void
  pending: boolean
}) {
  const t = useTranslations('admin')
  return (
    <Menu>
      <Menu.Trigger className="text-ink-muted hover:bg-surface-2 rounded-md p-1">
        <MoreVertical className="h-4 w-4" />
      </Menu.Trigger>
      <Menu.Popup>
        <Menu.Item disabled={pending} onClick={() => onResync(integration)}>
          {t('resync')}
        </Menu.Item>
      </Menu.Popup>
    </Menu>
  )
}

// Next.js 16：useSearchParams 需 Suspense 边界（静态导出场景）。
export default function AdminIntegrationsPage() {
  return (
    <Suspense fallback={null}>
      <AdminIntegrationsPageInner />
    </Suspense>
  )
}
