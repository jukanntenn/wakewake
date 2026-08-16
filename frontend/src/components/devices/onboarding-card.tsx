'use client'

// OnboardingCard：设备列表为空且 agent 未配对时的引导卡（specs/frontend/agent-onboarding.md）。
// ① 连接 Agent → ② 添加设备 → ③ 远程开机；当前步 = ①。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export function OnboardingCard() {
  const t = useTranslations('empty')
  const tDev = useTranslations('device')

  const steps = [t('onboarding.step1'), t('onboarding.step2'), t('onboarding.step3')]

  return (
    <div className="border-hairline bg-surface-1 shadow-card mx-auto max-w-xl rounded-lg border p-8 text-center">
      <div className="flex items-center justify-center gap-2 sm:gap-3">
        {steps.map((label, i) => (
          <div key={label} className="flex items-center gap-2 sm:gap-3">
            <div
              className={cn(
                'flex items-center gap-2 rounded-full border px-3 py-1.5 text-sm',
                i === 0 ? 'border-primary text-primary' : 'border-hairline text-ink-subtle',
              )}
            >
              <span
                className={cn(
                  'flex h-5 w-5 items-center justify-center rounded-full text-xs font-medium',
                  i === 0 ? 'bg-primary text-on-primary' : 'bg-surface-2 text-ink-subtle',
                )}
              >
                {i + 1}
              </span>
              <span className="whitespace-nowrap">{label}</span>
            </div>
            {i < steps.length - 1 && <span className="text-ink-subtle">—</span>}
          </div>
        ))}
      </div>
      <p className="text-ink-muted mx-auto mt-5 max-w-md text-sm leading-relaxed">
        {t('onboarding.desc')}
      </p>
      <Link href="/agents" className={cn(buttonVariants(), 'mt-5')}>
        {tDev('gate.cta')}
      </Link>
    </div>
  )
}
