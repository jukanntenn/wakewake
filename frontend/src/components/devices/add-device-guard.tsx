'use client'

// 添加设备前置拦截（specs/frontend/agent-onboarding.md 状态机）：
// agent 未配对 / 配额已满 / 非安全上下文三因共用一个轻量 Dialog。
// 按钮永远可点，点了永远得到解释——不做无声禁用。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { CircleAlert, ShieldAlert, Unplug } from 'lucide-react'
import { Button, buttonVariants } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'

export type AddDeviceGate = 'agent' | 'quota' | 'insecure'

// 前置状态机（specs/frontend/agent-onboarding.md，顺序固定，命中即返回）：
// agent 未配对 → 配额已满（limits 缺失时跳过，服务端兜底）→ 非安全上下文 → 放行。
export function resolveAddDeviceGate(input: {
  hasAgentPublicKey: boolean
  maxDevices?: number
  deviceCount: number
  isSecureContext: boolean
}): AddDeviceGate | null {
  if (!input.hasAgentPublicKey) return 'agent'
  if (input.maxDevices !== undefined && input.deviceCount >= input.maxDevices) return 'quota'
  if (!input.isSecureContext) return 'insecure'
  return null
}

export function AddDeviceGuardDialog({
  gate,
  maxDevices,
  onClose,
}: {
  gate: AddDeviceGate | null
  maxDevices?: number
  onClose: () => void
}) {
  const t = useTranslations('device')

  const meta = {
    agent: {
      icon: <Unplug className="text-warning h-5 w-5" />,
      title: t('gate.agentPendingTitle'),
    },
    quota: {
      icon: <CircleAlert className="text-ink-subtle h-5 w-5" />,
      title: t('gate.quotaTitle'),
    },
    insecure: {
      icon: <ShieldAlert className="text-ink-subtle h-5 w-5" />,
      title: t('gate.insecureTitle'),
    },
  }[gate ?? 'agent']

  return (
    <Dialog open={!!gate} onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {meta.icon}
            {meta.title}
          </DialogTitle>
        </DialogHeader>
        <p className="text-ink-muted text-sm leading-relaxed">
          {gate === 'quota' ? t('gate.quotaDesc', { max: String(maxDevices ?? '') }) : null}
          {gate === 'agent' ? t('gate.agentPendingDesc') : null}
          {gate === 'insecure' ? t('gate.insecureDesc') : null}
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t('gate.dismiss')}
          </Button>
          {gate === 'agent' && (
            <Link href="/agents" className={buttonVariants()} onClick={onClose}>
              {t('gate.cta')}
            </Link>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
