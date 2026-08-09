'use client'

// DeviceList：设备卡片网格（device-sync-v3 §14.2 双徽标 + §14.3 唤醒交互）。
// 投影徽标 = agent_online + projection_status；云徽标 = cloud_status + 漂移琥珀小标（agent 离线 UI 兜底）。

import { useCallback, useState } from 'react'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Pencil, Trash2, Power, Cloud, CloudOff } from 'lucide-react'
import { type Device } from '@/lib/api'
import { useDeleteDevice, useWakeDevice, pollCommandStatus } from '@/hooks/useDevices'

interface DeviceListProps {
  devices: Device[]
  onEdit: (device: Device) => void
}

export function DeviceList({ devices, onEdit }: DeviceListProps) {
  const t = useTranslations('device')
  const deleteMut = useDeleteDevice()
  const wakeMut = useWakeDevice()
  const [wakingIds, setWakingIds] = useState<Set<string>>(new Set())

  const onWake = useCallback(
    async (device: Device) => {
      // §14.3：agent 离线时禁用唤醒按钮（入口阻断，不等 60s 超时）
      if (!device.agent_online) {
        toast.error(t('agentOffline'))
        return
      }
      setWakingIds((prev) => new Set(prev).add(device.did))
      try {
        const commandId = await wakeMut.mutateAsync(device.did)
        const result = await pollCommandStatus(commandId, () => {})
        if (result.status === 'completed' && result.success) {
          toast.success(t('wakeSuccess'))
        } else if (result.status === 'expired') {
          toast.error(t('wakeError'))
        } else {
          toast.error(result.message || t('wakeError'))
        }
      } catch {
        toast.error(t('wakeError'))
      } finally {
        setWakingIds((prev) => {
          const next = new Set(prev)
          next.delete(device.did)
          return next
        })
      }
    },
    [wakeMut, t],
  )

  const onDelete = async (device: Device) => {
    if (!confirm(t('deleteConfirm'))) return
    try {
      await deleteMut.mutateAsync(device.did)
      toast.success(t('deleteSuccess'))
    } catch {
      toast.error(t('deleteFailed'))
    }
  }

  if (devices.length === 0) {
    return <p className="text-ink-muted">{t('noDevices')}</p>
  }

  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {devices.map((device) => (
        <div
          key={device.did}
          className="border-hairline bg-surface-1 shadow-card hover:shadow-hover rounded-lg border p-6 transition"
        >
          <div className="flex items-start justify-between">
            <div>
              <h3 className="text-ink font-medium">{device.name}</h3>
              <p className="text-ink-subtle mt-1 font-mono text-xs">{device.mac_display}</p>
              {device.description && (
                <p className="text-ink-muted mt-1 text-sm">{device.description}</p>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              <ProjectionBadge
                agentOnline={device.agent_online}
                projectionStatus={device.projection_status}
              />
              <CloudStatusIcon device={device} />
            </div>
          </div>
          <div className="mt-4 flex gap-2">
            <button
              onClick={() => onWake(device)}
              disabled={!device.agent_online || wakingIds.has(device.did)}
              className="bg-success text-ink hover:bg-success/90 flex items-center gap-1 rounded-md px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              title={!device.agent_online ? t('agentOffline') : undefined}
            >
              <Power className="h-3.5 w-3.5" />
              {wakingIds.has(device.did) ? t('waking') : t('wake')}
            </button>
            <button
              onClick={() => onEdit(device)}
              className="text-ink-muted hover:bg-surface-2 hover:text-ink rounded-md p-1.5 transition-colors"
              aria-label={t('edit')}
            >
              <Pencil className="h-4 w-4" />
            </button>
            <button
              onClick={() => onDelete(device)}
              className="text-ink-muted hover:bg-surface-2 hover:text-destructive rounded-md p-1.5 transition-colors"
              aria-label={t('delete')}
            >
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        </div>
      ))}
    </div>
  )
}

/// §14.2 漂移告警前端时间过滤：last_drift_at 非空且在 24h 内 → 仍展示。
/// 模块级函数（非组件内），避免组件渲染期调用 Date.now 触发 React 纯度规则。
function isDriftActive(lastDriftAt: string | null): boolean {
  if (!lastDriftAt) return false
  return Date.now() - new Date(lastDriftAt).getTime() < 24 * 60 * 60 * 1000
}

/// 投影徽标（§14.2 表 1）：agent_online + projection_status → 灰/黄/绿点 + 文案。
function ProjectionBadge({
  agentOnline,
  projectionStatus,
}: {
  agentOnline: boolean
  projectionStatus: Device['projection_status']
}) {
  const t = useTranslations('device')
  if (!agentOnline || projectionStatus === 'agent_offline') {
    return <span className="bg-ink-subtle h-2 w-2 rounded-full" title={t('agentOffline')} />
  }
  if (projectionStatus === 'syncing') {
    return <span className="bg-warning h-2 w-2 rounded-full" title={t('syncing')} />
  }
  return <span className="bg-success h-2 w-2 rounded-full" title={t('agentOnline')} />
}

/// 云徽标（§14.2 表 2）：cloud_status + 漂移琥珀小标（agent 离线 UI 兜底）。
/// 漂移告警前端按 now - last_drift_at < 24h 过滤显示（§14.2）。
function CloudStatusIcon({ device }: { device: Device }) {
  const t = useTranslations('device')

  // agent 离线 UI 兜底：云徽标优先显示 agent_offline 态（§14.2）
  if (!device.agent_online) {
    return (
      <span title={t('agentOffline')}>
        <CloudOff className="text-ink-subtle h-3.5 w-3.5" />
      </span>
    )
  }

  // 漂移告警展示规则（§14.2）：last_drift_at 非空 + 24h 内 → 琥珀小标叠加。
  // 纯前端时间过滤，在模块级函数计算（避免组件内调用 Date.now 触发 React 纯度 lint）。
  const driftActive = isDriftActive(device.last_drift_at)

  switch (device.cloud_status) {
    case 'no_integration':
      return (
        <span
          title={t('cloudNone')}
          className="border-hairline text-ink-subtle inline-flex h-4 w-4 items-center justify-center rounded-full border border-dashed"
        >
          <CloudOff className="h-3 w-3" />
        </span>
      )
    case 'not_observed':
      return (
        <span title={t('cloudNotObserved')}>
          <Cloud className="text-warning h-3.5 w-3.5" />
        </span>
      )
    case 'syncing':
      return (
        <span title={t('cloudSyncing')}>
          <Cloud className="text-warning h-3.5 w-3.5" />
        </span>
      )
    case 'synced':
      if (driftActive && device.last_drift_kind) {
        const driftTooltip =
          device.last_drift_kind === 'deleted' ? t('driftDeleted') : t('driftRenamed')
        return (
          <span className="relative" title={driftTooltip}>
            <Cloud className="text-success h-3.5 w-3.5" />
            <span className="bg-warning absolute -top-1 -right-1 h-1.5 w-1.5 rounded-full" />
          </span>
        )
      }
      return (
        <span title={t('cloudSynced')}>
          <Cloud className="text-success h-3.5 w-3.5" />
        </span>
      )
    case 'error':
      return (
        <span title={t('cloudError', { error: device.last_error ?? device.cloud_status })}>
          <Cloud className="text-destructive h-3.5 w-3.5" />
        </span>
      )
    default:
      return null
  }
}
