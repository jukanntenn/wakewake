'use client'

// 首页 = Devices 页（ui-ux-risk-control §3 合并决策）。
// 结构：健康摘要条（§3.3） + 标题+Add 按钮（§3.2） + 设备卡片网格（§3.5 四态）。
// AsyncView 状态机管理 loading/error/empty/data（§6）。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { Plus } from 'lucide-react'
import { useDevices } from '@/hooks/useDevices'
import { DeviceList } from '@/components/devices/DeviceList'
import { DeviceForm } from '@/components/devices/DeviceForm'
import { HealthSummaryBar } from '@/components/devices/health-summary-bar'
import { Button } from '@/components/ui/button'
import { AsyncView } from '@/components/ui/async-view'
import { EmptyState } from '@/components/ui/empty-state'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Wifi } from 'lucide-react'
import { type Device } from '@/lib/api'

export default function DevicesPage() {
  const t = useTranslations('device')
  const td = useTranslations('devices')
  const ten = useTranslations('empty')
  const query = useDevices()
  const [editTarget, setEditTarget] = useState<Device | null>(null)
  const [open, setOpen] = useState(false)

  const onEdit = (device: Device) => {
    setEditTarget(device)
    setOpen(true)
  }
  const onAdd = () => {
    setEditTarget(null)
    setOpen(true)
  }
  const onDone = () => {
    setOpen(false)
    setEditTarget(null)
  }

  return (
    <div className="space-y-6">
      {/* 健康摘要条（§3.3） */}
      <HealthSummaryBar />

      {/* 标题 + 操作（§3.2） */}
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{td('title')}</h1>
        <Dialog open={open} onOpenChange={setOpen}>
          <Button onClick={onAdd} className="flex items-center gap-1">
            <Plus className="h-4 w-4" />
            {t('addDevice')}
          </Button>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{editTarget ? t('edit') : t('create')}</DialogTitle>
            </DialogHeader>
            <DeviceForm device={editTarget} onDone={onDone} />
          </DialogContent>
        </Dialog>
      </div>

      {/* 设备卡片网格（AsyncView 状态机，§6） */}
      <AsyncView
        query={query}
        loadingVariant="cards"
        empty={
          <EmptyState
            icon={<Wifi />}
            title={ten('devices')}
            description={ten('devicesDesc')}
            action={
              <Button onClick={onAdd} className="flex items-center gap-1">
                <Plus className="h-4 w-4" />
                {ten('addDevice')}
              </Button>
            }
          />
        }
      >
        {(devices) => <DeviceList devices={devices} onEdit={onEdit} />}
      </AsyncView>
    </div>
  )
}
