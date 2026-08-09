'use client'

// Admin 跨用户 agent 页（/admin/agents）。AdminRoute 守卫。
// 列全部 agent（pending/online/offline + 归属 user），online agent 提供「强制断开」。

import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Loader2 } from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useAdminAgents, useDisconnectAgent } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'

export default function AdminAgentsPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const { data, isLoading } = useAdminAgents()
  const disconnectMut = useDisconnectAgent()
  const agents = data?.items ?? []

  const onDisconnect = async (id: number) => {
    try {
      await disconnectMut.mutateAsync(id)
      toast.success(t('disconnectSuccess'))
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  const statusBadge = (status: string) => {
    const map: Record<string, string> = {
      online: 'text-green-600 dark:text-green-400',
      offline: 'text-ink-subtle',
      pending: 'text-amber-600 dark:text-amber-400',
    }
    return map[status] ?? 'text-ink-muted'
  }
  const dotColor = (status: string) =>
    status === 'online' ? 'bg-green-500' : status === 'pending' ? 'bg-amber-500' : 'bg-gray-400'

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('agents')}</h1>

      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <p className="text-ink-muted mb-4 text-sm">{t('agentsDesc')}</p>
        {isLoading ? (
          <div className="text-ink-subtle flex h-32 items-center justify-center text-sm">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('loading')}
          </div>
        ) : agents.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-hairline text-ink-muted border-b text-left">
                  <th className="py-2 pr-4 font-medium">{t('colAgent')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colOwner')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colStatus')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colLastSeen')}</th>
                  <th className="py-2 pr-4 font-medium">{t('actions')}</th>
                </tr>
              </thead>
              <tbody>
                {agents.map((a) => (
                  <tr key={a.id} className="border-hairline border-b">
                    <td className="py-2 pr-4">
                      <div className="text-ink font-medium">{a.name}</div>
                      <div className="text-ink-subtle text-xs">{a.aid.slice(0, 12)}…</div>
                    </td>
                    <td className="text-ink-muted py-2 pr-4">{a.user_email}</td>
                    <td className={`py-2 pr-4 font-medium ${statusBadge(a.status)}`}>
                      <span className="inline-flex items-center">
                        <span className={`mr-1.5 h-2 w-2 rounded-full ${dotColor(a.status)}`} />
                        {t(`agent_${a.status}` as 'agent_online')}
                      </span>
                    </td>
                    <td className="text-ink-subtle py-2 pr-4">
                      {a.last_seen ? new Date(a.last_seen).toLocaleString() : '—'}
                    </td>
                    <td className="py-2 pr-4">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={a.status !== 'online' || disconnectMut.isPending}
                        onClick={() => onDisconnect(a.id)}
                        title={a.status !== 'online' ? t('disconnectDisabledHint') : undefined}
                      >
                        {t('disconnect')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="border-hairline text-ink-subtle flex h-32 items-center justify-center rounded-sm border border-dashed text-sm">
            {t('emptyAgents')}
          </div>
        )}
      </div>
    </div>
  )
}
