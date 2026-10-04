'use client'

// Agent 状态信息条（specs/frontend/agent-onboarding.md）：pending 引导重配对 / offline 告知待同步。
// 只告知，不阻断任何操作。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { RelativeTime } from '@/components/ui/relative-time'

export function AgentNoticeBar({
  variant,
  lastSeen,
}: {
  variant: 'pending' | 'offline'
  lastSeen?: string | null
}) {
  const t = useTranslations('device')

  return (
    <div className="border-hairline bg-surface-1 flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border px-4 py-3">
      <StatusIndicator
        color={variant === 'pending' ? 'active' : 'neutral'}
        pulse={variant === 'pending'}
      />
      <span className="text-ink text-sm font-medium">
        {variant === 'pending' ? t('notice.pendingTitle') : t('notice.offlineTitle')}
        {variant === 'offline' && lastSeen && (
          <span className="text-ink-subtle">
            {' '}
            · <RelativeTime date={lastSeen} />
          </span>
        )}
      </span>
      <span className="text-ink-muted text-sm">
        {variant === 'pending' ? t('notice.pendingDesc') : t('notice.offlineDesc')}
      </span>
      <Link
        href="/agents"
        className="text-primary text-sm underline-offset-4 hover:underline ltr:ml-auto rtl:mr-auto"
      >
        {t('viewAgent')} →
      </Link>
    </div>
  )
}
