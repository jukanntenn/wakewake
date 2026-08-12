'use client'

// ProjectionBadge：投影徽标（§4.3/§12.7，§14.2 徽标 1）。
// 输入 agent_online + projection_status，内部按规范表渲染，调用方无法传错。
// 渲染规则（不可篡改）：
//   !agent_online → neutral 灰点
//   syncing       → active 琥珀点 + 脉冲
//   synced        → success 绿点

import { useTranslations } from 'next-intl'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

export interface ProjectionBadgeProps {
  agentOnline: boolean
  projectionStatus: 'syncing' | 'synced' | 'agent_offline'
}

export function ProjectionBadge({ agentOnline, projectionStatus }: ProjectionBadgeProps) {
  const t = useTranslations('device')

  let indicator
  let tooltipKey: string
  if (!agentOnline || projectionStatus === 'agent_offline') {
    indicator = <StatusIndicator color="neutral" />
    tooltipKey = 'agentOffline'
  } else if (projectionStatus === 'syncing') {
    indicator = <StatusIndicator color="active" pulse />
    tooltipKey = 'syncing'
  } else {
    indicator = <StatusIndicator color="success" />
    tooltipKey = 'agentOnline'
  }

  return (
    <Tooltip>
      <TooltipTrigger render={<span className="inline-flex">{indicator}</span>} />
      <TooltipContent>{t(tooltipKey)}</TooltipContent>
    </Tooltip>
  )
}
