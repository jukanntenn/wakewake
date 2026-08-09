'use client'

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { Plus } from 'lucide-react'
import { useDevices } from '@/hooks/useDevices'
import { DeviceList } from '@/components/devices/DeviceList'
import { DeviceForm } from '@/components/devices/DeviceForm'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { type Device } from '@/lib/api'

export default function DevicesPage() {
  const t = useTranslations('device')
  const td = useTranslations('devices')
  const { data: devices, isLoading } = useDevices()
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
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{td('title')}</h1>
        <Dialog open={open} onOpenChange={setOpen}>
          <Button onClick={onAdd} className="flex items-center gap-1">
            <Plus className="h-4 w-4" />
            {t('create')}
          </Button>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{editTarget ? t('edit') : t('create')}</DialogTitle>
            </DialogHeader>
            <DeviceForm device={editTarget} onDone={onDone} />
          </DialogContent>
        </Dialog>
      </div>
      {isLoading ? (
        <p className="text-ink-muted">{t('loading')}</p>
      ) : (
        <DeviceList devices={devices ?? []} onEdit={onEdit} />
      )}
    </div>
  )
}
