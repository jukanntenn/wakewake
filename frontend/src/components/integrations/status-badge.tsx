'use client'

// IntegrationStatusBadge：集成状态徽标（§4.3，§14.4 七态表）。

import { useTranslations } from 'next-intl'
import { StatusIndicator, type StatusColor } from '@/components/ui/status-indicator'

export interface IntegrationStatusBadgeProps {
  status:
    | 'not_connected'
    | 'connecting'
    | 'connected'
    | 'disconnected'
    | 'error'
    | 'disabled'
    | 'agent_offline'
  lastError?: string | null
}

export function IntegrationStatusBadge({ status, lastError }: IntegrationStatusBadgeProps) {
  const t = useTranslations('integrations')

  const map: Record<
    IntegrationStatusBadgeProps['status'],
    { color: StatusColor; pulse?: boolean; label: string }
  > = {
    connected: { color: 'success', label: t('stateConnected') },
    connecting: { color: 'active', pulse: true, label: t('stateConnecting') },
    error: { color: 'error', label: t('stateError') },
    disconnected: { color: 'warning', label: t('stateDisconnected') },
    disabled: { color: 'neutral', label: t('stateDisabled') },
    agent_offline: { color: 'neutral', label: t('stateAgentOffline') },
    not_connected: { color: 'neutral', label: t('notConnected') },
  }

  const cfg = map[status]
  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <StatusIndicator color={cfg.color} pulse={cfg.pulse} aria-label={cfg.label} />
      <span className="text-ink-muted">
        {cfg.label}
        {status === 'error' && lastError ? `: ${lastError}` : ''}
      </span>
    </span>
  )
}
