'use client'

// § TRUST:威胁模型 Q&A(2×2)。完全诚实口径——含"至多重放旧密文"的披露,
// 断言与 backend/crates/agent/src/crypto.rs 及 README 安全模型一致。

import { useTranslations } from 'next-intl'
import { SectionHeading } from './section-heading'

export function TrustSection() {
  const t = useTranslations('landing.trust')
  const items = [
    { q: t('q1'), a: t('a1') },
    { q: t('q2'), a: t('a2') },
    { q: t('q3'), a: t('a3') },
    { q: t('q4'), a: t('a4') },
  ]

  return (
    <section id="security" className="scroll-mt-20">
      <div className="mx-auto max-w-[1200px] px-4 py-16 md:px-6 md:py-20">
        <SectionHeading ns="landing.trust" />

        <div className="mx-auto mt-10 grid max-w-4xl gap-4 sm:grid-cols-2 md:mt-14">
          {items.map(({ q, a }) => (
            <div key={q} className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
              <h3 className="text-ink text-[15px] leading-snug font-semibold">{q}</h3>
              <p className="text-ink-muted mt-2.5 text-sm leading-relaxed">{a}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
