'use client'

// 用户侧 Wakes 页（ui-ux-risk-control §14.11）。
// 游标分页 Load More（用户侧不是 admin 的 offset）。
// 内联 StatusBadge → 规范 <WakeStatusBadge>。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { RefreshCw } from 'lucide-react'
import { useWakes } from '@/hooks/useWakes'
import { Button } from '@/components/ui/button'
import { LoadingState } from '@/components/ui/loading-state'
import { WakeStatusBadge } from '@/components/wakes/status-badge'
import { DataTable, type Column } from '@/components/ui/data-table'
import { type Wake } from '@/lib/api'
import { RelativeTime } from '@/components/ui/relative-time'

export default function WakesPage() {
  const t = useTranslations('wakes')
  const [before, setBefore] = useState<string | undefined>(undefined)
  const { data, isLoading, isFetching, refetch } = useWakes(before)

  const items = data?.items ?? []
  const total = data?.total ?? 0
  const hasMore = items.length < total && items.length > 0

  const loadMore = () => {
    const last = items[items.length - 1]
    if (last) setBefore(last.created_at)
  }

  const columns: Column<Wake>[] = [
    { key: 'device_name', label: t('device') },
    { key: 'type', label: t('type') },
    { key: 'status', label: t('statusCol') },
    { key: 'created_at', label: t('time') },
  ]

  const renderCell = (wake: Wake, key: string) => {
    if (key === 'device_name') return <span className="text-ink">{wake.device_name}</span>
    if (key === 'type')
      return (
        <span className="text-ink-muted">
          {wake.type === 'wol' ? t('typeManual') : t('typeVoice')}
        </span>
      )
    if (key === 'status') return <WakeStatusBadge status={wake.status} message={wake.message} />
    if (key === 'created_at')
      return (
        <span className="text-ink-subtle">
          <RelativeTime date={wake.created_at} />
        </span>
      )
    return null
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>
        <Button
          variant="outline"
          size="sm"
          onClick={() => refetch()}
          disabled={isFetching}
          className="gap-1.5"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${isFetching ? 'animate-spin' : ''}`} />
          {t('refresh')}
        </Button>
      </div>

      {isLoading ? (
        <LoadingState variant="rows" />
      ) : items.length === 0 ? (
        <p className="text-ink-muted py-8 text-center">{t('empty')}</p>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={items}
            rowKey={(w) => w.id}
            render={renderCell}
            mobile={{
              primary: (w) => <span className="text-ink font-medium">{w.device_name}</span>,
              secondary: [
                { key: 'type', label: t('type') },
                { key: 'status', label: t('statusCol') },
                { key: 'created_at', label: t('time') },
              ],
            }}
          />
          {hasMore && (
            <div className="flex justify-center">
              <Button onClick={loadMore} disabled={isFetching}>
                {isFetching ? t('loading') : t('loadMore')}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
