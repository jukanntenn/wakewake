'use client'

// Admin 概览页（ui-ux-risk-control §9.2）。
// 三层信息架构：概览层（健康仪表盘）+ 管理层（治理/运维入口）+ 系统层（维护）。
// 健康时展示聚合 stats；异常时替换为告警列表 + 预设过滤跳转。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { ChevronRight, Activity, Users, Cpu, HardDrive, Zap, Wrench } from 'lucide-react'
import { useAdminStats } from '@/hooks/useAdmin'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { LoadingState } from '@/components/ui/loading-state'

export default function AdminOverviewPage() {
  const t = useTranslations('admin')
  const tHealth = useTranslations('health')
  const { data: stats, isLoading } = useAdminStats()

  if (isLoading || !stats) {
    return (
      <div className="space-y-6">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>
        <LoadingState variant="rows" count={3} />
      </div>
    )
  }

  const hasIssues = stats.devices_sync_error > 0

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>

      {/* 概览层：健康仪表盘 */}
      <div className="border-hairline bg-surface-1 rounded-lg border p-5">
        {hasIssues ? (
          <div className="space-y-2">
            <div className="text-warning flex items-center gap-1.5 text-sm font-medium">
              <StatusIndicator color="warning" />
              {tHealth('devicesNeedAttention', { count: stats.devices_sync_error })}
            </div>
            {stats.devices_sync_error > 0 && (
              <Link
                href="/admin/devices?cloud_status=error"
                className="text-ink-muted hover:bg-surface-2 flex items-center justify-between rounded-md px-2 py-1.5 text-sm"
              >
                <span>• {stats.devices_sync_error} device sync errors</span>
                <ChevronRight className="h-4 w-4" />
              </Link>
            )}
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-1.5 text-sm font-medium">
              <StatusIndicator color="success" />
              <span className="text-ink">{tHealth('healthy')}</span>
            </div>
            <div className="text-ink-muted flex flex-wrap gap-x-6 gap-y-1 text-sm">
              <span>
                {stats.users} {t('statUsers')} ({t('statActiveUsers', { n: stats.active_users })})
              </span>
              <span>{t('statOnlineAgents', { n: stats.online_agents, total: stats.agents })}</span>
            </div>
            <div className="text-ink-muted flex flex-wrap gap-x-6 gap-y-1 text-sm">
              <span>
                {stats.devices} {t('statDevices')} · {stats.devices_sync_error} {t('statSyncError')}
              </span>
              <span>{t('statIntegrations', { n: stats.integrations })}</span>
            </div>
          </div>
        )}
      </div>

      {/* 治理层 */}
      <section className="space-y-2">
        <h2 className="text-ink-muted text-xs font-medium tracking-wider uppercase">
          {t('userManagement')}
        </h2>
        <div className="border-hairline bg-surface-1 divide-y divide-[var(--color-hairline)] overflow-hidden rounded-lg border">
          <AdminEntry
            href="/admin/users"
            icon={<Users className="h-4 w-4" />}
            title={t('users')}
            desc={t('userManagementDesc')}
          />
          <AdminEntry
            href="/admin/activity"
            icon={<Activity className="h-4 w-4" />}
            title={t('activity')}
            desc={t('activityDesc')}
          />
        </div>
      </section>

      {/* 运维层 */}
      <section className="space-y-2">
        <h2 className="text-ink-muted text-xs font-medium tracking-wider uppercase">
          {t('operationsTitle')}
        </h2>
        <div className="border-hairline bg-surface-1 divide-y divide-[var(--color-hairline)] overflow-hidden rounded-lg border">
          <AdminEntry
            href="/admin/devices"
            icon={<HardDrive className="h-4 w-4" />}
            title={t('devices')}
            desc={t('devicesDesc')}
          />
          <AdminEntry
            href="/admin/agents"
            icon={<Cpu className="h-4 w-4" />}
            title={t('agents')}
            desc={t('agentsDesc')}
          />
          <AdminEntry
            href="/admin/integrations"
            icon={<Zap className="h-4 w-4" />}
            title={t('integrations')}
            desc={t('integrationsDesc')}
          />
          <AdminEntry
            href="/admin/wakes"
            icon={<Activity className="h-4 w-4" />}
            title={t('wakes')}
            desc={t('wakesDesc')}
          />
        </div>
      </section>

      {/* 系统层 */}
      <section className="space-y-2">
        <h2 className="text-ink-muted text-xs font-medium tracking-wider uppercase">
          {t('systemTitle')}
        </h2>
        <div className="border-hairline bg-surface-1 overflow-hidden rounded-lg border">
          <AdminEntry
            href="/admin/maintenance"
            icon={<Wrench className="h-4 w-4" />}
            title={t('maintenance.title')}
            desc={t('maintenanceDesc')}
          />
        </div>
      </section>
    </div>
  )
}

function AdminEntry({
  href,
  icon,
  title,
  desc,
}: {
  href: string
  icon: React.ReactNode
  title: string
  desc: string
}) {
  return (
    <Link
      href={href}
      className="hover:bg-surface-2 flex items-center justify-between px-4 py-3 transition-colors"
    >
      <div className="flex items-center gap-3">
        <span className="text-ink-muted">{icon}</span>
        <div>
          <p className="text-ink text-sm font-medium">{title}</p>
          <p className="text-ink-muted text-xs">{desc}</p>
        </div>
      </div>
      <ChevronRight className="text-ink-subtle h-4 w-4" />
    </Link>
  )
}
