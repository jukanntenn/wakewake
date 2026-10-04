'use client'

// useSystemHealth：系统健康聚合 hook（§3.4/§13.8）。
// 纯前端 reduce useDevices 结果，零后端改动。
// 状态阈值（§11.A.3）：healthy=issueCount 0；warning=1-4；error=≥5 或全部 agent 离线。

import { useMemo } from 'react'
import { useTranslations } from 'next-intl'
import type { Device } from '@/lib/api'
import { useDevices } from '@/hooks/useDevices'

export type HealthStatus = 'healthy' | 'warning' | 'error'

export interface DeviceIssue {
  id: string
  kind: 'device_error' | 'integration_error' | 'agent_offline'
  label: string
  href: string
  severity: number // 排序用（1=安全最高，见 §11.A.2）
}

export interface SystemHealth {
  status: HealthStatus
  deviceCount: number
  readyCount: number
  issueCount: number
  issues: DeviceIssue[]
  agentOnline: boolean
  cloudSyncOk: boolean
}

function reduceHealth(devices: Device[], t: ReturnType<typeof useTranslations>): SystemHealth {
  const deviceCount = devices.length
  const readyCount = devices.filter((d) => d.agent_online).length
  const cloudSyncOk = devices.every((d) => d.cloud_status !== 'error')
  // 单租户场景：agent_online 聚合（任一在线即 agentOnline=true）
  const agentOnline = devices.some((d) => d.agent_online)

  const issues: DeviceIssue[] = []
  for (const d of devices) {
    if (d.cloud_status === 'error') {
      issues.push({
        id: `device-error-${d.did}`,
        kind: 'device_error',
        label: t('health.cloudSyncError') + ' — ' + d.name,
        href: '/devices',
        severity: 4, // §11.A.2 排序：device_error 最低
      })
    }
  }

  // §11.A.2 排序（按 severity 升序，安全最高优先；此处用户侧只有 device_error）
  issues.sort((a, b) => a.severity - b.severity)

  // §11.A.3 阈值：全部 agent 离线（readyCount==0 且有设备）→ error
  const allAgentsOffline = deviceCount > 0 && readyCount === 0
  const issueCount = issues.length
  let status: HealthStatus
  if (issueCount === 0 && !allAgentsOffline) {
    status = 'healthy'
  } else if (issueCount >= 5 || allAgentsOffline) {
    status = 'error'
  } else {
    status = 'warning'
  }

  return { status, deviceCount, readyCount, issueCount, issues, agentOnline, cloudSyncOk }
}

export function useSystemHealth(): SystemHealth {
  const t = useTranslations()
  const { data: devices } = useDevices()
  return useMemo(() => reduceHealth(devices ?? [], t), [devices, t])
}
