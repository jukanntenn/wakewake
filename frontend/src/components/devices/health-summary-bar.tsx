'use client'

// HealthSummaryBar：首页健康摘要条（ui-ux-risk-control §3.3/§12.8）。
// 三维度摘要（设备就绪 / agent 在线 / 云同步），数据来自 useSystemHealth。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { TriangleAlert } from 'lucide-react'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { useSystemHealth } from '@/hooks/useSystemHealth'

export function HealthSummaryBar() {
  const t = useTranslations('health')
  const health = useSystemHealth()

  if (health.status === 'healthy' && health.deviceCount > 0) {
    return (
      <div className="border-hairline bg-surface-1 rounded-lg border p-4">
        <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-sm">
          <span className="flex items-center gap-1.5">
            <StatusIndicator color="success" />
            <span className="text-ink font-medium">
              {t('devicesReady', { count: health.readyCount })}
            </span>
          </span>
          <span className="text-ink-muted">
            {health.agentOnline ? t('agentOnline') : t('agentOffline')}
          </span>
          <span className="text-ink-muted">
            {health.cloudSyncOk ? t('cloudSyncOk') : t('cloudSyncError')}
          </span>
        </div>
      </div>
    )
  }

  if (health.issues.length > 0) {
    return (
      <div className="border-warning/30 bg-warning/5 rounded-lg border p-4">
        <div className="text-warning flex items-center gap-1.5 text-sm font-medium">
          <TriangleAlert className="h-4 w-4" />
          {t('deviceNeedsAttention', { count: health.issues.length })}
        </div>
        <ul className="mt-2 space-y-1 text-sm">
          {health.issues.slice(0, 3).map((issue) => (
            <li key={issue.id} className="text-ink-muted flex items-center justify-between gap-2">
              <span>• {issue.label}</span>
              <Link href={issue.href} className="text-primary shrink-0 text-xs">
                {t('view' as never)}
              </Link>
            </li>
          ))}
          {health.issues.length > 3 && (
            <li className="text-ink-subtle text-xs">
              {t('moreIssues', { count: health.issues.length - 3 })}
            </li>
          )}
        </ul>
      </div>
    )
  }

  // 无设备时不显示摘要条（空态占据）
  return null
}
