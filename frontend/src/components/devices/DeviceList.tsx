'use client'

// DeviceList：设备卡片网格（ui-ux-risk-control §3.5 四态 + §12.6 精确规格）。
// 四态由 agent_online 主导：沉睡=opacity-60；告警=border-l-2 border-warning。
// 双独立徽标用规范组件（ProjectionBadge/CloudBadge）。
// 删除走 L3 ConfirmDialog（§5.4），唤醒按钮 text-ink（共识 3，a11y 达标）。

import { useCallback, useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Pencil, Trash2, Power, MoreHorizontal } from 'lucide-react'
import { cn } from '@/lib/utils'
import { type Device } from '@/lib/api'
import { useDeleteDevice, useWakeDevice, pollCommandStatus } from '@/hooks/useDevices'
import { ProjectionBadge } from '@/components/devices/projection-badge'
import { CloudBadge } from '@/components/devices/cloud-badge'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { DeviceDetailDialog } from '@/components/devices/DeviceDetailDialog'

interface DeviceListProps {
  devices: Device[]
  onEdit: (device: Device) => void
}

export function DeviceList({ devices, onEdit }: DeviceListProps) {
  const t = useTranslations('device')
  const deleteMut = useDeleteDevice()
  const wakeMut = useWakeDevice()
  const [wakingIds, setWakingIds] = useState<Set<string>>(new Set())
  const [deleteTarget, setDeleteTarget] = useState<Device | null>(null)
  const [detailTarget, setDetailTarget] = useState<Device | null>(null)

  const onWake = useCallback(
    async (device: Device) => {
      if (!device.agent_online) return
      setWakingIds((prev) => new Set(prev).add(device.did))
      try {
        const commandId = await wakeMut.mutateAsync(device.did)
        const result = await pollCommandStatus(commandId, () => {})
        if (result.status === 'completed' && result.success) {
          toast.success(t('wakeSuccess'))
        } else if (result.status === 'expired') {
          toast.error(t('wakeTimeout'))
        } else {
          toast.error(t('wakeFailed', { message: result.message ?? '' }))
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

  const handleConfirmDelete = async () => {
    if (!deleteTarget) return
    await deleteMut.mutateAsync(deleteTarget.did)
    toast.success(t('deleteSuccess'))
    setDeleteTarget(null)
  }

  return (
    <>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {devices.map((device) => (
          <div
            key={device.did}
            className={cn(
              'border-hairline bg-surface-1 shadow-card rounded-lg border p-5 transition md:p-6',
              !device.agent_online && 'opacity-60',
              device.cloud_status === 'error' && 'border-warning border-l-2',
            )}
          >
            {/* 头部：图标 + 名称 + 徽标 */}
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 items-start gap-3">
                <DeviceIcon online={device.agent_online} />
                <div className="min-w-0">
                  <h3 className="text-ink truncate font-medium">{device.name}</h3>
                  <p className="text-ink-subtle mt-0.5 font-mono text-xs">{device.mac_display}</p>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <ProjectionBadge
                  agentOnline={device.agent_online}
                  projectionStatus={device.projection_status}
                />
                <CloudBadge device={device} />
              </div>
            </div>

            {/* 描述（虚线分隔） */}
            {device.description && (
              <>
                <div className="border-hairline my-3 border-t border-dashed" />
                <p className="text-ink-muted text-sm">{device.description}</p>
              </>
            )}

            {/* 离线态状态行（附 Agent 页去向，agent-onboarding.md） */}
            {!device.agent_online && (
              <p className="text-ink-subtle mt-3 text-xs">
                ○ {t('agentOffline')} · {t('wakeUnavailable' as never)} ·{' '}
                <Link href="/agents" className="hover:text-ink underline underline-offset-2">
                  {t('viewAgent')}
                </Link>
              </p>
            )}

            {/* 唤醒按钮（全宽，text-ink 共识 3） */}
            <button
              onClick={() => onWake(device)}
              disabled={!device.agent_online || wakingIds.has(device.did)}
              className="bg-success text-ink hover:bg-success/90 mt-4 flex w-full items-center justify-center gap-1.5 rounded-md py-2 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Power className="h-3.5 w-3.5" />
              {wakingIds.has(device.did) ? t('waking') : t('wake')}
            </button>

            {/* 操作图标（右下弱化）：§3.7 详情入口 ··· + Edit + Delete */}
            <div className="mt-3 flex justify-end gap-1">
              <button
                onClick={() => setDetailTarget(device)}
                aria-label={t('details')}
                className="text-ink-muted hover:bg-surface-2 hover:text-ink rounded-md p-1.5 transition"
              >
                <MoreHorizontal className="h-4 w-4" />
              </button>
              <button
                onClick={() => onEdit(device)}
                aria-label={t('edit')}
                className="text-ink-muted hover:bg-surface-2 hover:text-ink rounded-md p-1.5 transition"
              >
                <Pencil className="h-4 w-4" />
              </button>
              <button
                onClick={() => setDeleteTarget(device)}
                aria-label={t('delete')}
                className="text-ink-muted hover:bg-surface-2 hover:text-destructive rounded-md p-1.5 transition"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* L3 删除确认弹窗（§5.4） */}
      <ConfirmDialog
        open={!!deleteTarget}
        onConfirm={handleConfirmDelete}
        onClose={() => setDeleteTarget(null)}
        variant="danger"
        title={t('deleteDialogTitle')}
        description={t('deleteDialogDesc', { name: deleteTarget?.name ?? '' })}
        confirmText={t('delete')}
        cancelText={t('cancel')}
      />

      {/* §3.7 设备详情面板（三区 + Edit/Delete 双入口） */}
      <DeviceDetailDialog
        device={detailTarget}
        onClose={() => setDetailTarget(null)}
        onEdit={onEdit}
        onDelete={(d) => setDeleteTarget(d)}
      />
    </>
  )
}

function DeviceIcon({ online }: { online: boolean }) {
  // §3.6：在线 💻 / 离线 💤
  return (
    <div
      className={cn(
        'flex h-10 w-10 items-center justify-center rounded-md text-xl',
        online ? 'bg-surface-2' : 'bg-surface-2 opacity-60',
      )}
    >
      {online ? '💻' : '💤'}
    </div>
  )
}
