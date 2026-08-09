'use client'

// Wakes 页面：唤醒历史游标分页列表（api-design.md §1.4 唤醒历史）。
// ?before=<created_at> 游标分页（上一页最后一条 created_at）。
// 手动刷新按钮 + 首页 10s 自动刷新（useWakes）。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { RefreshCw } from 'lucide-react'
import { useWakes } from '@/hooks/useWakes'
import { Button } from '@/components/ui/button'

export default function WakesPage() {
  const t = useTranslations('wakes')
  const [before, setBefore] = useState<string | undefined>(undefined)
  const { data, isLoading, isFetching, refetch } = useWakes(before)

  const items = data?.items ?? []
  const total = data?.total ?? 0
  const hasMore = items.length < total && items.length > 0

  const loadMore = () => {
    const last = items[items.length - 1]
    if (last) {
      setBefore(last.created_at)
    }
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

      <div className="border-hairline bg-surface-1 shadow-card overflow-hidden rounded-lg border">
        <table className="w-full">
          <thead>
            <tr className="border-hairline text-ink-muted border-b text-left text-sm">
              <th className="px-4 py-3 font-medium">{t('device')}</th>
              <th className="px-4 py-3 font-medium">{t('type')}</th>
              <th className="px-4 py-3 font-medium">{t('statusCol')}</th>
              <th className="px-4 py-3 font-medium">{t('time')}</th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <tr>
                <td colSpan={4} className="text-ink-muted px-4 py-8 text-center">
                  {t('loading')}
                </td>
              </tr>
            ) : items.length === 0 ? (
              <tr>
                <td colSpan={4} className="text-ink-muted px-4 py-8 text-center">
                  {t('empty')}
                </td>
              </tr>
            ) : (
              items.map((wake) => (
                <tr key={wake.id} className="border-hairline hover:bg-surface-2 border-b text-sm">
                  <td className="text-ink px-4 py-3">{wake.device_name}</td>
                  <td className="text-ink-muted px-4 py-3">
                    {wake.type === 'wol' ? t('typeManual') : t('typeVoice')}
                  </td>
                  <td className="px-4 py-3">
                    <StatusBadge status={wake.status} />
                  </td>
                  <td className="text-ink-subtle px-4 py-3">
                    {new Date(wake.created_at).toLocaleString()}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {hasMore && (
        <div className="flex justify-center">
          <Button onClick={loadMore} disabled={isFetching}>
            {isFetching ? t('loading') : t('loadMore')}
          </Button>
        </div>
      )}
    </div>
  )
}

function StatusBadge({ status }: { status: 'success' | 'failed' | 'expired' }) {
  const t = useTranslations('wakes')
  const color =
    status === 'success' ? 'bg-success' : status === 'expired' ? 'bg-warning' : 'bg-destructive'
  const label =
    status === 'success'
      ? t('statusSuccess')
      : status === 'expired'
        ? t('statusExpired')
        : t('statusFailed')
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className={`h-2 w-2 rounded-full ${color}`} />
      <span className="text-ink-muted">{label}</span>
    </span>
  )
}
