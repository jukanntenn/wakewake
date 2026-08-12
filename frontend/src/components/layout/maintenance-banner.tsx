'use client'

// MaintenanceBanner：维护模式全局横幅（ui-ux-risk-control §2.4/§12.15）。
// 仅在 maintenance.enabled 且 mode !== 'full' 时展示（full 模式用户被拦截看不到）。
// 数据来自 GET /health/maintenance（公开端点）。

import { useQuery } from '@tanstack/react-query'
import { useTranslations } from 'next-intl'
import { Wrench } from 'lucide-react'
import { api } from '@/lib/api'

export function MaintenanceBanner() {
  const t = useTranslations('maintenance')
  const { data } = useQuery({
    queryKey: ['maintenance-status'],
    queryFn: () => api.health.maintenance(),
    refetchInterval: 60_000, // 1min 轮询
    staleTime: 30_000,
  })

  if (!data?.enabled || data.mode === 'full') {
    return null
  }

  return (
    <div className="bg-warning/10 border-warning/30 border-b px-4 py-2 md:px-6">
      <p className="text-ink-muted flex items-center justify-center gap-2 text-sm">
        <Wrench className="text-warning h-4 w-4 shrink-0" />
        <span className="truncate">{data.message || t('banner')}</span>
      </p>
    </div>
  )
}
