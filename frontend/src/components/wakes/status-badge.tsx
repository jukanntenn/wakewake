'use client'

// WakeStatusBadge：唤醒状态徽标（§4.3，3 态：success/failed/expired）。

import { useTranslations } from 'next-intl'
import { StatusIndicator, type StatusColor } from '@/components/ui/status-indicator'

export interface WakeStatusBadgeProps {
  status: 'success' | 'failed' | 'expired'
  message?: string | null
}

export function WakeStatusBadge({ status, message }: WakeStatusBadgeProps) {
  const t = useTranslations('wakes')

  const map: Record<WakeStatusBadgeProps['status'], { color: StatusColor; label: string }> = {
    success: { color: 'success', label: t('statusSuccess') },
    failed: { color: 'error', label: t('statusFailed') },
    expired: { color: 'warning', label: t('statusExpired') },
  }

  const cfg = map[status]
  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <StatusIndicator color={cfg.color} aria-label={cfg.label} />
      <span className="text-ink-muted">
        {cfg.label}
        {status === 'failed' && message ? `: ${message}` : ''}
      </span>
    </span>
  )
}
