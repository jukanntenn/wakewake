'use client'

// DeviceDetailDialog：§3.7 设备详情面板（IDENTITY/CONNECTION/CLOUD SYNC 三区）。
// 只读 + 操作入口（Edit/Delete 双入口，§3.6/§3.7）；Edit 走外层 DeviceForm Dialog
// （§11.B.3 不在详情面板内 inline 编辑，避免两种编辑范式）。
// MAC 行只读并标 (masked)（§14.3 产品决策：UI 默认禁用 MAC 编辑）。

import { type ReactNode } from 'react'
import { useTranslations } from 'next-intl'
import { Pencil, Trash2 } from 'lucide-react'
import { type Device } from '@/lib/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { ProjectionBadge } from '@/components/devices/projection-badge'
import { CloudBadge } from '@/components/devices/cloud-badge'

interface DeviceDetailDialogProps {
  device: Device | null
  onClose: () => void
  onEdit: (device: Device) => void
  onDelete: (device: Device) => void
}

export function DeviceDetailDialog({ device, onClose, onEdit, onDelete }: DeviceDetailDialogProps) {
  const t = useTranslations('device')
  const open = !!device

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          {/* 详情面板标题=设备名（§3.7 mockup 头部仅名称 + ✕，由 DialogContent 提供） */}
          <DialogTitle className="truncate pr-8">{device?.name}</DialogTitle>
        </DialogHeader>

        {device && (
          <div className="space-y-5">
            {/* IDENTITY：MAC 只读(masked) + Description + Created */}
            <Zone title={t('detailIdentity')}>
              <Row label={t('colMac')}>
                <span className="font-mono text-xs">{device.mac_display}</span>{' '}
                <span className="text-ink-subtle text-xs">{t('masked')}</span>
              </Row>
              <Row label={t('colDescription')}>
                <span className="text-sm">{device.description || t('noDescription')}</span>
              </Row>
              <Row label={t('colCreated')}>
                <span className="text-sm">{new Date(device.created_at).toLocaleString()}</span>
              </Row>
            </Zone>

            {/* CONNECTION：Agent 名 + Last seen（投影徽标内联表示在线/同步/离线） */}
            <Zone title={t('detailConnection')}>
              <Row label={t('colAgent')}>
                <span className="flex items-center justify-end gap-1.5">
                  <ProjectionBadge
                    agentOnline={device.agent_online}
                    projectionStatus={device.projection_status}
                  />
                  <span className="text-sm">{device.agent_name ?? t('never')}</span>
                </span>
              </Row>
              <Row label={t('colLastSeen')}>
                <span className="text-sm">
                  {device.agent_last_seen
                    ? new Date(device.agent_last_seen).toLocaleString()
                    : t('never')}
                </span>
              </Row>
            </Zone>

            {/* CLOUD SYNC：Status + Last sync + Last drift */}
            <Zone title={t('detailCloudSync')}>
              <Row label={t('colStatus')}>
                <CloudBadge device={device} />
              </Row>
              <Row label={t('colLastSync')}>
                <span className="text-sm">
                  {device.cloud_observed_at
                    ? new Date(device.cloud_observed_at).toLocaleString()
                    : t('never')}
                </span>
              </Row>
              <Row label={t('colLastDrift')}>
                <span className="text-sm">
                  {device.last_drift_at
                    ? new Date(device.last_drift_at).toLocaleString()
                    : t('noDrift')}
                </span>
              </Row>
            </Zone>

            {/* §3.6/§3.7 双入口：详情面板内重复卡片右下的 Edit/Delete */}
            <div className="border-hairline flex flex-col gap-2 border-t pt-4 sm:flex-row">
              <button
                onClick={() => {
                  onEdit(device)
                  onClose()
                }}
                className="text-ink-muted hover:bg-surface-2 hover:text-ink flex flex-1 items-center justify-center gap-1.5 rounded-md py-2 text-sm font-medium transition"
              >
                <Pencil className="h-4 w-4" />
                {t('edit')}
              </button>
              <button
                onClick={() => {
                  onDelete(device)
                  onClose()
                }}
                className="text-ink-muted hover:bg-surface-2 hover:text-destructive flex flex-1 items-center justify-center gap-1.5 rounded-md py-2 text-sm font-medium transition"
              >
                <Trash2 className="h-4 w-4" />
                {t('delete')}
              </button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

/// 三区标题（小号大写弱化色，与 §3.7 mockup 一致）。
function Zone({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h4 className="text-ink-subtle mb-2 text-xs font-semibold tracking-wider uppercase">
        {title}
      </h4>
      <div className="space-y-1.5">{children}</div>
    </section>
  )
}

/// 键值行：label 左对齐弱化，value 右对齐主色（dl 语义用 div 避免嵌套约束）。
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-ink-muted shrink-0 text-sm">{label}</span>
      <span className="text-ink text-right text-sm">{children}</span>
    </div>
  )
}
