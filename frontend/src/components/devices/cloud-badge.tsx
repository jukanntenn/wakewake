'use client'

// CloudBadge：云徽标（§4.3/§12.7，§14.2 徽标 2，7 行 UI 优先级表）。
// 严格按优先级从上到下匹配即返回，不可篡改。
// 漂移 24h 前端过滤（模块级 isDriftActive 纯函数，避免组件内 Date.now 触发 React 纯度）。

import { Cloud, CloudOff } from 'lucide-react'
import { useTranslations } from 'next-intl'
import type { Device } from '@/lib/api'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

export interface CloudBadgeProps {
  device: Pick<
    Device,
    'agent_online' | 'cloud_status' | 'last_drift_at' | 'last_drift_kind' | 'last_error'
  >
}

// §14.2 漂移前端时间过滤：last_drift_at 非空且 24h 内 → 仍展示。
// 模块级函数（非组件内），避免组件渲染期调用 Date.now 触发 React 纯度 lint。
export function isDriftActive(lastDriftAt: string | null): boolean {
  if (!lastDriftAt) return false
  return Date.now() - new Date(lastDriftAt).getTime() < 24 * 60 * 60 * 1000
}

export function CloudBadge({ device }: CloudBadgeProps) {
  const t = useTranslations('device')
  const { agent_online, cloud_status, last_drift_at, last_drift_kind, last_error } = device

  let indicator
  let tooltip: string

  if (!agent_online) {
    indicator = <StatusIndicator color="neutral" icon={<CloudOff />} />
    tooltip = t('agentOffline')
  } else if (cloud_status === 'no_integration') {
    // 第 2 行：无集成/disabled → 虚线灰云
    indicator = (
      <span className="border-hairline text-ink-subtle inline-flex h-4 w-4 items-center justify-center rounded-full border border-dashed">
        <CloudOff className="h-3 w-3" />
      </span>
    )
    tooltip = t('cloudNone')
  } else if (cloud_status === 'not_observed') {
    indicator = <StatusIndicator color="active" icon={<Cloud />} />
    tooltip = t('cloudNotObserved')
  } else if (cloud_status === 'syncing') {
    indicator = <StatusIndicator color="active" icon={<Cloud />} pulse />
    tooltip = t('cloudSyncing')
  } else if (cloud_status === 'synced') {
    const driftActive = isDriftActive(last_drift_at)
    if (driftActive && last_drift_kind) {
      indicator = (
        <span className="relative inline-flex">
          <Cloud className="text-success h-3.5 w-3.5" />
          <span className="bg-warning absolute -top-1 -right-1 h-1.5 w-1.5 rounded-full" />
        </span>
      )
      tooltip = last_drift_kind === 'deleted' ? t('driftDeleted') : t('driftRenamed')
    } else {
      indicator = <StatusIndicator color="success" icon={<Cloud />} />
      tooltip = t('cloudSynced')
    }
  } else {
    // error
    indicator = <StatusIndicator color="error" icon={<Cloud />} />
    tooltip = t('cloudError', { error: last_error ?? cloud_status })
  }

  return (
    <Tooltip>
      <TooltipTrigger render={<span className="inline-flex">{indicator}</span>} />
      <TooltipContent>{tooltip}</TooltipContent>
    </Tooltip>
  )
}
