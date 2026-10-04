'use client'

// StatusPill：状态胶囊（ui-ux-risk-control §2.2③/§12.4）。
// 三态：健康=8px 绿点；异常=琥珀三角+数字；严重=红叉。
// 点击跳首页健康摘要（用户）/ Admin 概览（admin）。
// 桌面端健康态可加文字（lg+）。

import { TriangleAlert, XCircle } from 'lucide-react'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { useSystemHealth } from '@/hooks/useSystemHealth'
import { useAuthStore } from '@/stores/auth'

export function StatusPill() {
  const t = useTranslations('status')
  const health = useSystemHealth()
  const router = useRouter()
  const isSuperuser = useAuthStore((s) => s.user?.is_superuser === true)

  const target = isSuperuser ? '/admin' : '/devices'

  if (health.status === 'healthy') {
    return (
      <button
        onClick={() => router.push(target)}
        className="flex items-center"
        aria-label={t('healthy')}
        role="status"
      >
        <span className="bg-success inline-block h-2 w-2 rounded-full" />
        <span className="text-ink-muted ml-1 hidden text-xs lg:inline">{t('healthy')}</span>
      </button>
    )
  }

  if (health.status === 'warning') {
    return (
      <button
        onClick={() => router.push(target)}
        className="text-warning flex items-center gap-1 text-xs font-medium"
        aria-label={t('issues', { count: health.issueCount })}
        role="status"
      >
        <TriangleAlert className="h-3.5 w-3.5" />
        <span>{health.issueCount}</span>
      </button>
    )
  }

  return (
    <button
      onClick={() => router.push('/admin')}
      className="text-destructive flex items-center"
      aria-label={t('serviceDegraded')}
      role="status"
    >
      <XCircle className="h-4 w-4" />
    </button>
  )
}
