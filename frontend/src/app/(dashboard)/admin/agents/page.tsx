'use client'

// Admin Agents 页（ui-ux-risk-control §9.7）。
// 调查型列表：DataTable + 搜索 + Status 过滤（客户端，后端不支持）+ offset 分页。
// 操作：Disconnect（L1，仅 online；调用 useDisconnectAgent(id)）。

import { Suspense, useMemo } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, MoreVertical } from 'lucide-react'
import { ApiError, type AdminAgent } from '@/lib/api'
import { useAdminAgents, useDisconnectAgent } from '@/hooks/useAdmin'
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

function AdminAgentsPageInner() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const router = useRouter()
  const searchParams = useSearchParams()

  // URL query 同步（§11.B.4）。
  const q = searchParams.get('q') ?? ''
  const statusFilter = searchParams.get('status') ?? 'all' // all/online/offline/pending
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? '10')

  const { data: resp, isLoading } = useAdminAgents({ q, page, page_size: pageSize })
  const total = resp?.total ?? 0

  const disconnectMut = useDisconnectAgent()

  // 客户端过滤：status（后端不支持 status 过滤，前端筛）。
  const filteredAgents = useMemo(() => {
    const items = resp?.items ?? []
    if (statusFilter === 'all') return items
    return items.filter((a) => a.status === statusFilter)
  }, [resp?.items, statusFilter])

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/agents?${params.toString()}`)
  }

  const onDisconnect = async (a: AdminAgent) => {
    try {
      await disconnectMut.mutateAsync(a.id)
      toast.success(t('disconnectSuccess'))
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  // Status 单元格：online=success；offline=neutral；pending=active 脉冲（§9.7）。
  const statusCell = (status: AdminAgent['status']) => {
    const map: Record<
      AdminAgent['status'],
      { color: 'success' | 'neutral' | 'active'; label: string }
    > = {
      online: { color: 'success', label: t('agent_online') },
      offline: { color: 'neutral', label: t('agent_offline') },
      pending: { color: 'active', label: t('agent_pending') },
    }
    const cfg = map[status]
    return (
      <span className="inline-flex items-center gap-1.5 text-sm">
        <StatusIndicator color={cfg.color} pulse={status === 'pending'} aria-label={cfg.label} />
        <span className="text-ink-muted">{cfg.label}</span>
      </span>
    )
  }

  const columns: Column<AdminAgent>[] = [
    { key: 'name', label: t('colAgent') },
    { key: 'user_email', label: t('colOwner') },
    { key: 'status', label: t('colStatus') },
    { key: 'last_seen', label: t('colLastSeen') },
    { key: 'actions', label: t('actions' as never) ?? '' },
  ]

  const renderCell = (a: AdminAgent, key: string) => {
    if (key === 'name') {
      return (
        <div>
          <div className="text-ink font-medium">{a.name}</div>
          <div className="text-ink-subtle text-xs">{a.aid.slice(0, 12)}…</div>
        </div>
      )
    }
    if (key === 'user_email') {
      return <span className="text-ink-muted text-sm">{a.user_email}</span>
    }
    if (key === 'status') {
      return statusCell(a.status)
    }
    if (key === 'last_seen') {
      return (
        <span className="text-ink-muted text-sm">
          <RelativeTime date={a.last_seen} />
        </span>
      )
    }
    if (key === 'actions') {
      return (
        <AgentsActionsMenu
          agent={a}
          onDisconnect={onDisconnect}
          pending={disconnectMut.isPending}
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
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('agents')}</h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchAgents' as never) ?? 'Search agent or user...'}
          defaultValue={q}
          onChange={(e) => updateQuery({ q: e.target.value, page: 1 })}
        />
        <Select
          value={statusFilter}
          onValueChange={(v) => updateQuery({ status: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="online">{t('agent_online' as never) ?? 'Online'}</SelectItem>
            <SelectItem value="offline">{t('agent_offline' as never) ?? 'Offline'}</SelectItem>
            <SelectItem value="pending">{t('agent_pending' as never) ?? 'Pending'}</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/agents')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={filteredAgents}
        rowKey={(a) => a.id}
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">
            {t('emptyAgents' as never) ?? 'No agents'}
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
          primary: (a) => <span className="text-ink font-medium">{a.user_email}</span>,
          secondary: [
            { key: 'status', label: t('colStatus') },
            { key: 'last_seen', label: t('colLastSeen') },
          ],
          actions: (a) => (
            <AgentsActionsMenu
              agent={a}
              onDisconnect={onDisconnect}
              pending={disconnectMut.isPending}
            />
          ),
        }}
      />
    </div>
  )
}

function AgentsActionsMenu({
  agent,
  onDisconnect,
  pending,
}: {
  agent: AdminAgent
  onDisconnect: (a: AdminAgent) => void
  pending: boolean
}) {
  const t = useTranslations('admin')
  const isOnline = agent.status === 'online'
  return (
    <Menu>
      <Menu.Trigger className="text-ink-muted hover:bg-surface-2 rounded-md p-1">
        <MoreVertical className="h-4 w-4" />
      </Menu.Trigger>
      <Menu.Popup>
        <Menu.Item
          disabled={!isOnline || pending}
          title={isOnline ? undefined : t('disconnectDisabledHint')}
          onClick={() => onDisconnect(agent)}
        >
          {t('disconnect')}
        </Menu.Item>
      </Menu.Popup>
    </Menu>
  )
}

// Next.js 16：useSearchParams 需 Suspense 边界（静态导出场景）。
export default function AdminAgentsPage() {
  return (
    <Suspense fallback={null}>
      <AdminAgentsPageInner />
    </Suspense>
  )
}
