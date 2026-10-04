'use client'

// Admin 概览页（ui-ux-risk-control §9.2）。
// 三层信息架构：概览层（健康仪表盘）+ 管理层（治理/运维入口）+ 系统层（维护）。
// 健康时展示聚合 stats；异常时替换为告警列表 + 预设过滤跳转。
// 风控区块（admin-risk-controls WRFC）：注册速率 / 发信消耗 / 失败登录聚合
// 一眼扫见，top 失败 IP 直连封禁（看见 → 处置一条流）。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import {
  ChevronRight,
  Activity,
  Users,
  Cpu,
  HardDrive,
  Zap,
  Wrench,
  ShieldAlert,
  ShieldBan,
  Mail,
} from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useAdminStats, useRiskOverview, useAddIpBan } from '@/hooks/useAdmin'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { LoadingState } from '@/components/ui/loading-state'
import { Button } from '@/components/ui/button'

export default function AdminOverviewPage() {
  const t = useTranslations('admin')
  const tHealth = useTranslations('health')
  const tErr = useTranslations('error')
  const { data: stats, isLoading } = useAdminStats()
  const { data: risk } = useRiskOverview()
  const banMut = useAddIpBan()

  const onBan = async (ip: string) => {
    try {
      // 默认 24h TTL：处置快、误伤可自愈；确属长期恶意再在封禁页升级为永久。
      await banMut.mutateAsync({ target: ip, ttl_hours: 24 })
      toast.success(t('ipBans.added'))
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

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

      {/* 风控区块：滥用信号一览 + top 失败 IP 一键封禁 */}
      <section className="space-y-2">
        <h2 className="text-ink-muted flex items-center gap-1.5 text-xs font-medium tracking-wider uppercase">
          <ShieldAlert className="h-3.5 w-3.5" />
          {t('risk')}
        </h2>
        <div className="border-hairline bg-surface-1 space-y-4 rounded-lg border p-5">
          {risk ? (
            <>
              <div className="text-ink-muted grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-3 lg:grid-cols-4">
                <span>
                  {t('riskRegistrations24h')}:{' '}
                  <span className="text-ink font-medium">{risk.registrations_24h}</span>
                </span>
                <span>
                  {t('riskRegistrations7d')}:{' '}
                  <span className="text-ink font-medium">{risk.registrations_7d}</span>
                </span>
                <span>
                  {t('riskUnverified')}:{' '}
                  <span className="text-ink font-medium">{risk.unverified_count}</span>
                  {risk.oldest_unverified_age_hours !== null && (
                    <span className="text-ink-subtle">
                      {' '}
                      ({t('riskOldest', { n: risk.oldest_unverified_age_hours })})
                    </span>
                  )}
                </span>
                <span>
                  {t('riskFailedLogins')}:{' '}
                  <span className="text-ink font-medium">{risk.failed_logins_24h}</span>
                </span>
                <span>
                  {t('riskEmailsToday')}:{' '}
                  <span className="text-ink font-medium">
                    {risk.mailer.sent.register + risk.mailer.sent.resend + risk.mailer.sent.reset}
                  </span>
                  {risk.mailer.blocked.register +
                    risk.mailer.blocked.resend +
                    risk.mailer.blocked.reset >
                    0 && (
                    <span className="text-warning">
                      {' '}
                      (
                      {risk.mailer.blocked.register +
                        risk.mailer.blocked.resend +
                        risk.mailer.blocked.reset}{' '}
                      {t('riskBlocked')})
                    </span>
                  )}
                  {!risk.mailer.enabled && (
                    <span className="text-destructive"> · {t('mailer.off')}</span>
                  )}
                </span>
                <span>
                  {t('riskPow')}:{' '}
                  <span className="text-ink font-medium">{risk.pow_difficulty}</span>
                </span>
                <span>
                  {t('riskIpBans')}:{' '}
                  <span className="text-ink font-medium">{risk.ip_ban_count}</span>
                </span>
                <span>
                  {t('riskRateLimited')}:{' '}
                  <span className="text-ink font-medium">{risk.rate_limited_since_start}</span>
                </span>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <div className="space-y-1.5">
                  <p className="text-ink-muted text-xs font-medium">{t('riskTopIps')}</p>
                  {risk.top_failed_ips.length === 0 ? (
                    <p className="text-ink-subtle text-xs">{t('riskNoData')}</p>
                  ) : (
                    risk.top_failed_ips.map((r) => (
                      <div key={r.ip} className="flex items-center justify-between gap-2 text-sm">
                        <span className="text-ink font-mono">{r.ip}</span>
                        <span className="text-ink-muted text-xs">
                          {r.failures} · {r.distinct_emails} {t('riskTargets')}
                        </span>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onBan(r.ip)}
                          disabled={banMut.isPending}
                        >
                          {t('riskBan')}
                        </Button>
                      </div>
                    ))
                  )}
                </div>
                <div className="space-y-1.5">
                  <p className="text-ink-muted text-xs font-medium">{t('riskTopEmails')}</p>
                  {risk.top_failed_emails.length === 0 ? (
                    <p className="text-ink-subtle text-xs">{t('riskNoData')}</p>
                  ) : (
                    risk.top_failed_emails.map((r) => (
                      <div
                        key={r.email}
                        className="flex items-center justify-between gap-2 text-sm"
                      >
                        <span className="text-ink max-w-56 truncate">{r.email}</span>
                        <span className="text-ink-muted text-xs">
                          {r.failures} · {r.distinct_ips} IP
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <Link
                href="/admin/ip-bans"
                className="text-ink-muted hover:text-ink flex items-center gap-1 text-sm"
              >
                {t('riskViewAll')}
                <ChevronRight className="h-4 w-4" />
              </Link>
            </>
          ) : (
            <LoadingState variant="rows" count={2} />
          )}
        </div>
      </section>

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
        <div className="border-hairline bg-surface-1 divide-y divide-[var(--color-hairline)] overflow-hidden rounded-lg border">
          <AdminEntry
            href="/admin/maintenance"
            icon={<Wrench className="h-4 w-4" />}
            title={t('maintenance.title')}
            desc={t('maintenanceDesc')}
          />
          <AdminEntry
            href="/admin/mailer"
            icon={<Mail className="h-4 w-4" />}
            title={t('mailer.title')}
            desc={t('mailer.desc')}
          />
          <AdminEntry
            href="/admin/ip-bans"
            icon={<ShieldBan className="h-4 w-4" />}
            title={t('ipBans.title')}
            desc={t('ipBans.desc')}
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
