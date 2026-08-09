'use client'

// Admin 跨用户设备页（/admin/devices）。AdminRoute 守卫。
// device-sync-v3：列全部设备（带派生 cloud_status + 归属 user），error/syncing 行高亮，
// 提供「强制重同步」操作（方案 A：bump projection_version + 断开 agent）。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Loader2 } from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useAdminDevices, useResyncDevice } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'

type CloudFilter = 'all' | 'not_observed' | 'syncing' | 'synced' | 'error'

export default function AdminDevicesPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const [filter, setFilter] = useState<CloudFilter>('all')
  const params = filter === 'all' ? {} : { cloud_status: filter }
  const { data, isLoading } = useAdminDevices(params)
  const resyncMut = useResyncDevice()
  const devices = data?.items ?? []

  const onResync = async (did: string) => {
    try {
      await resyncMut.mutateAsync(did)
      toast.success(t('resyncSuccess'))
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  const cloudBadge = (status: string) => {
    const map: Record<string, string> = {
      synced: 'text-green-600 dark:text-green-400',
      syncing: 'text-amber-600 dark:text-amber-400',
      not_observed: 'text-amber-600 dark:text-amber-400',
      error: 'text-red-600 dark:text-red-400',
      no_integration: 'text-ink-subtle',
    }
    return map[status] ?? 'text-ink-muted'
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('devices')}</h1>
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value as CloudFilter)}
          className="border-hairline bg-surface-1 text-ink focus:ring-primary h-10 rounded-md border px-3 text-sm focus:ring-2 focus:outline-none"
        >
          <option value="all">{t('filterAll')}</option>
          <option value="synced">{t('filterSynced')}</option>
          <option value="syncing">{t('filterSyncing')}</option>
          <option value="not_observed">not_observed</option>
          <option value="error">error</option>
        </select>
      </div>

      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        {isLoading ? (
          <div className="text-ink-subtle flex h-32 items-center justify-center text-sm">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('loading')}
          </div>
        ) : devices.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-hairline text-ink-muted border-b text-left">
                  <th className="py-2 pr-4 font-medium">{t('colDevice')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colOwner')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colSyncStatus')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colUpdated')}</th>
                  <th className="py-2 pr-4 font-medium">{t('actions')}</th>
                </tr>
              </thead>
              <tbody>
                {devices.map((d) => (
                  <tr
                    key={d.did}
                    className={
                      'border-hairline border-b ' +
                      (d.cloud_status !== 'synced' && d.cloud_status !== 'no_integration'
                        ? 'bg-amber-50/50 dark:bg-amber-950/10'
                        : '')
                    }
                  >
                    <td className="py-2 pr-4">
                      <div className="text-ink font-medium">{d.name}</div>
                      <div className="text-ink-subtle text-xs">{d.did.slice(0, 12)}…</div>
                    </td>
                    <td className="text-ink-muted py-2 pr-4">{d.user_email}</td>
                    <td className={`py-2 pr-4 font-medium ${cloudBadge(d.cloud_status)}`}>
                      {d.cloud_status}
                      {d.last_error && (
                        <div className="text-ink-subtle text-xs">{d.last_error}</div>
                      )}
                    </td>
                    <td className="text-ink-subtle py-2 pr-4">
                      {d.updated_at ? new Date(d.updated_at).toLocaleString() : '—'}
                    </td>
                    <td className="py-2 pr-4">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={resyncMut.isPending}
                        onClick={() => onResync(d.did)}
                      >
                        {t('resync')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="border-hairline text-ink-subtle flex h-32 items-center justify-center rounded-sm border border-dashed text-sm">
            {t('emptyDevices')}
          </div>
        )}
      </div>
    </div>
  )
}
