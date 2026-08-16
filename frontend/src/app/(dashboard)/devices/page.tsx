'use client'

// 首页 = Devices 页（ui-ux-risk-control §3 合并决策）。
// 结构：健康摘要条（§3.3） + Agent 信息条 + 标题+Add 按钮（§3.2，前置门控） + 设备卡片网格（§3.5）。
// 添加设备前置状态机（specs/frontend/agent-onboarding.md）：
//   agent 未配对 → 配额已满（limits 经 /me 下发）→ 非安全上下文 → 打开表单。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { Plus } from 'lucide-react'
import { useDevices } from '@/hooks/useDevices'
import { useDefaultAgent } from '@/hooks/useAgents'
import { useAuthStore } from '@/stores/auth'
import { DeviceList } from '@/components/devices/DeviceList'
import { DeviceForm } from '@/components/devices/DeviceForm'
import { HealthSummaryBar } from '@/components/devices/health-summary-bar'
import { AgentNoticeBar } from '@/components/devices/agent-notice-bar'
import { OnboardingCard } from '@/components/devices/onboarding-card'
import {
  AddDeviceGuardDialog,
  resolveAddDeviceGate,
  type AddDeviceGate,
} from '@/components/devices/add-device-guard'
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
  const { data: agent } = useDefaultAgent()
  const limits = useAuthStore((s) => s.user?.limits)
  const [editTarget, setEditTarget] = useState<Device | null>(null)
  const [open, setOpen] = useState(false)
  const [gate, setGate] = useState<AddDeviceGate | null>(null)

  const maxDevices = limits?.max_devices
  const deviceCount = query.data?.length ?? 0

  const onEdit = (device: Device) => {
    setEditTarget(device)
    setOpen(true)
  }
  const onAdd = () => {
    const gate = resolveAddDeviceGate({
      hasAgentPublicKey: !!agent?.public_key,
      maxDevices,
      deviceCount,
      isSecureContext: typeof window === 'undefined' ? true : window.isSecureContext,
    })
    if (gate) {
      setGate(gate)
      return
    }
    setEditTarget(null)
    setOpen(true)
  }
  const onDone = () => {
    setOpen(false)
    setEditTarget(null)
  }

  const agentPending = agent?.status === 'pending'

  return (
    <div className="space-y-6">
      {/* 健康摘要条（§3.3） */}
      <HealthSummaryBar />

      {/* Agent 状态信息条：pending（已有设备时的重配对引导，空列表由 onboarding 卡负责）
          或 offline（放行但告知待同步） */}
      {((agentPending && deviceCount > 0) || agent?.status === 'offline') && (
        <AgentNoticeBar
          variant={agentPending ? 'pending' : 'offline'}
          lastSeen={agent?.last_seen}
        />
      )}

      {/* 标题 + 配额徽标 + 操作（§3.2） */}
      <div className="flex items-center justify-between">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">
          {td('title')}
          {maxDevices !== undefined && deviceCount > 0 && (
            <span className="text-ink-subtle ml-2 align-middle font-mono text-base">
              {deviceCount}/{maxDevices}
            </span>
          )}
        </h1>
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

      {/* 设备卡片网格（AsyncView 状态机，§6）；空 + pending → onboarding 引导卡 */}
      <AsyncView
        query={query}
        loadingVariant="cards"
        empty={
          agentPending ? (
            <OnboardingCard />
          ) : (
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
          )
        }
      >
        {(devices) => <DeviceList devices={devices} onEdit={onEdit} />}
      </AsyncView>

      <AddDeviceGuardDialog gate={gate} maxDevices={maxDevices} onClose={() => setGate(null)} />
    </div>
  )
}
