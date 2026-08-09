'use client'

// Admin 审计日志页（/admin/audit-log）。AdminRoute 守卫。
// 列 admin_actions 审计记录（action 过滤 + 分页），风控追溯「谁对什么做了什么」。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { Loader2 } from 'lucide-react'
import { useAuditLog } from '@/hooks/useAdmin'

const ACTIONS = [
  'all',
  'user.disable',
  'user.enable',
  'user.reset_password',
  'user.verify_email',
  'device.resync',
  'integration.resync',
  'agent.disconnect',
] as const
type ActionFilter = (typeof ACTIONS)[number]

const PAGE_SIZE = 30

export default function AuditLogPage() {
  const t = useTranslations('admin')
  const [action, setAction] = useState<ActionFilter>('all')
  const [page, setPage] = useState(1)
  const params =
    action === 'all' ? { page, page_size: PAGE_SIZE } : { action, page, page_size: PAGE_SIZE }
  const { data, isLoading } = useAuditLog(params)
  const entries = data?.items ?? []
  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  const targetLabel = (e: (typeof entries)[number]) => {
    if (e.target_user_id) return `user#${e.target_user_id}`
    if (e.target_agent_id) return `agent#${e.target_agent_id}`
    if (e.target_device_did) return `device ${e.target_device_did.slice(0, 8)}`
    return '—'
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('auditLog')}</h1>
        <select
          value={action}
          onChange={(e) => {
            setAction(e.target.value as ActionFilter)
            setPage(1)
          }}
          className="border-hairline bg-surface-1 text-ink focus:ring-primary h-10 rounded-md border px-3 text-sm focus:ring-2 focus:outline-none"
        >
          {ACTIONS.map((a) => (
            <option key={a} value={a}>
              {a === 'all' ? t('filterAll') : a}
            </option>
          ))}
        </select>
      </div>

      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        {isLoading ? (
          <div className="text-ink-subtle flex h-32 items-center justify-center text-sm">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('loading')}
          </div>
        ) : entries.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-hairline text-ink-muted border-b text-left">
                  <th className="py-2 pr-4 font-medium">{t('colTime')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colActor')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colAction')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colTarget')}</th>
                  <th className="py-2 pr-4 font-medium">{t('colDetail')}</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((e) => (
                  <tr key={e.id} className="border-hairline border-b">
                    <td className="text-ink-subtle py-2 pr-4 whitespace-nowrap">
                      {new Date(e.created_at).toLocaleString()}
                    </td>
                    <td className="text-ink py-2 pr-4">{e.actor_email}</td>
                    <td className="py-2 pr-4">
                      <code className="bg-surface-2 text-ink rounded px-1.5 py-0.5 text-xs">
                        {e.action}
                      </code>
                    </td>
                    <td className="text-ink-muted py-2 pr-4">{targetLabel(e)}</td>
                    <td className="text-ink-subtle py-2 pr-4">
                      {Object.keys(e.detail).length > 0 ? JSON.stringify(e.detail) : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="border-hairline text-ink-subtle flex h-32 items-center justify-center rounded-sm border border-dashed text-sm">
            {t('emptyAudit')}
          </div>
        )}

        {totalPages > 1 && (
          <div className="text-ink-muted mt-4 flex items-center justify-between text-sm">
            <span>
              {t('pagination', { page, total: totalPages })}（{total}）
            </span>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page <= 1}
                className="border-hairline rounded-md border px-3 py-1 disabled:opacity-40"
              >
                ←
              </button>
              <button
                type="button"
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                disabled={page >= totalPages}
                className="border-hairline rounded-md border px-3 py-1 disabled:opacity-40"
              >
                →
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
