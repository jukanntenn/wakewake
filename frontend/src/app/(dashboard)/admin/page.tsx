'use client'

// Admin 风控概览页（/admin）。
// 全局聚合 stats 卡片 + 最近审计日志。管理员快速识别异常（卡住的 sync / 离线 agent）。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { Loader2, Shield, Users, Cpu, HardDrive, Activity, AlertTriangle } from 'lucide-react'
import { useAdminStats, useAuditLog } from '@/hooks/useAdmin'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

function StatCard({
  label,
  value,
  hint,
  alert,
}: {
  label: string
  value: number | string
  hint?: string
  alert?: boolean
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-ink-muted text-sm font-medium tracking-normal">
          {label}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div
          className={`text-3xl font-semibold ${alert ? 'text-amber-600 dark:text-amber-400' : 'text-ink'}`}
        >
          {value}
        </div>
        {hint ? <p className="text-ink-subtle mt-1 text-xs">{hint}</p> : null}
      </CardContent>
    </Card>
  )
}

export default function AdminPage() {
  const t = useTranslations('admin')
  const { data: stats, isLoading } = useAdminStats()
  const { data: audit } = useAuditLog({ page_size: 10 })

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Shield className="text-ink-muted h-6 w-6" />
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>
      </div>

      {isLoading || !stats ? (
        <div className="text-ink-subtle flex h-40 items-center justify-center text-sm">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          {t('loading')}
        </div>
      ) : (
        <>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <StatCard
              label={t('statUsers')}
              value={stats.users}
              hint={t('statActiveUsers', { n: stats.active_users })}
            />
            <StatCard
              label={t('statAgents')}
              value={stats.agents}
              hint={t('statOnlineAgents', { n: stats.online_agents })}
            />
            <StatCard
              label={t('statDevices')}
              value={stats.devices}
              hint={t('statIntegrations', { n: stats.integrations })}
            />
            <StatCard label={t('statWakes')} value={stats.wakes} />
            <StatCard
              label={t('statSyncing')}
              value={stats.devices_syncing}
              hint={t('statSyncingHint')}
              alert={stats.devices_syncing > 0}
            />
            <StatCard
              label={t('statSyncError')}
              value={stats.devices_sync_error}
              hint={t('statSyncErrorHint')}
              alert={stats.devices_sync_error > 0}
            />
          </div>

          {(stats.devices_sync_error > 0 || stats.devices_syncing > 0) && (
            <Card className="border-amber-300 dark:border-amber-700">
              <CardContent className="flex items-start gap-3 p-4">
                <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" />
                <div className="text-sm">
                  <p className="text-ink font-medium">{t('attentionTitle')}</p>
                  <p className="text-ink-muted mt-1">{t('attentionDesc')}</p>
                  <div className="mt-2 flex gap-3">
                    <Link href="/admin/devices" className="text-primary hover:underline">
                      {t('devices')} →
                    </Link>
                    <Link href="/admin/agents" className="text-primary hover:underline">
                      {t('agents')} →
                    </Link>
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <Link href="/admin/users" className="hover:opacity-80">
              <Card>
                <CardContent className="flex items-center gap-4 p-6">
                  <Users className="text-ink-muted h-8 w-8" />
                  <div>
                    <p className="text-ink font-medium">{t('users')}</p>
                    <p className="text-ink-muted text-sm">{t('manageUsers')}</p>
                  </div>
                </CardContent>
              </Card>
            </Link>
            <Link href="/admin/devices" className="hover:opacity-80">
              <Card>
                <CardContent className="flex items-center gap-4 p-6">
                  <HardDrive className="text-ink-muted h-8 w-8" />
                  <div>
                    <p className="text-ink font-medium">{t('devices')}</p>
                    <p className="text-ink-muted text-sm">{t('manageDevices')}</p>
                  </div>
                </CardContent>
              </Card>
            </Link>
            <Link href="/admin/agents" className="hover:opacity-80">
              <Card>
                <CardContent className="flex items-center gap-4 p-6">
                  <Cpu className="text-ink-muted h-8 w-8" />
                  <div>
                    <p className="text-ink font-medium">{t('agents')}</p>
                    <p className="text-ink-muted text-sm">{t('manageAgents')}</p>
                  </div>
                </CardContent>
              </Card>
            </Link>
            <Link href="/admin/audit-log" className="hover:opacity-80">
              <Card>
                <CardContent className="flex items-center gap-4 p-6">
                  <Activity className="text-ink-muted h-8 w-8" />
                  <div>
                    <p className="text-ink font-medium">{t('auditLog')}</p>
                    <p className="text-ink-muted text-sm">{t('manageAuditLog')}</p>
                  </div>
                </CardContent>
              </Card>
            </Link>
          </div>

          <Card>
            <CardHeader>
              <CardTitle className="text-ink text-lg">{t('recentActions')}</CardTitle>
            </CardHeader>
            <CardContent>
              {audit && audit.items.length > 0 ? (
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-hairline text-ink-muted border-b text-left">
                        <th className="py-2 pr-4 font-medium">{t('colTime')}</th>
                        <th className="py-2 pr-4 font-medium">{t('colActor')}</th>
                        <th className="py-2 pr-4 font-medium">{t('colAction')}</th>
                        <th className="py-2 pr-4 font-medium">{t('colTarget')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {audit.items.map((e) => (
                        <tr key={e.id} className="border-hairline border-b">
                          <td className="text-ink-subtle py-2 pr-4">
                            {new Date(e.created_at).toLocaleString()}
                          </td>
                          <td className="text-ink py-2 pr-4">{e.actor_email}</td>
                          <td className="text-ink-muted py-2 pr-4">
                            <code className="bg-surface-2 rounded px-1.5 py-0.5 text-xs">
                              {e.action}
                            </code>
                          </td>
                          <td className="text-ink-subtle py-2 pr-4">
                            {e.target_user_id
                              ? `user#${e.target_user_id}`
                              : e.target_agent_id
                                ? `agent#${e.target_agent_id}`
                                : e.target_device_did
                                  ? `device ${e.target_device_did.slice(0, 8)}`
                                  : '—'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <p className="text-ink-subtle py-6 text-center text-sm">{t('emptyAudit')}</p>
              )}
              <div className="mt-3 text-right">
                <Link href="/admin/audit-log" className="text-primary text-sm hover:underline">
                  {t('viewAllAudit')} →
                </Link>
              </div>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  )
}
