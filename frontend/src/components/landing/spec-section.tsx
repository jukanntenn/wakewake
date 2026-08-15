'use client'

// § SPEC:数字带(display 级 mono 大数字)+ 双列文字清单(无 icon)。
// 数字全部来自 README:100k 设计目标 / ~1.2KB 每连接 / 2GB VPS / 8 语言。

import { useTranslations } from 'next-intl'
import { SectionHeading } from './section-heading'

const STATS = [
  { value: '100,000', key: 'stat1' as const },
  { value: '~1.2 KB', key: 'stat2' as const },
  { value: '2 GB', key: 'stat3' as const },
  { value: '8', key: 'stat4' as const },
]

const ITEMS = Array.from({ length: 9 }, (_, i) => `item${i + 1}` as const)

export function SpecSection() {
  const t = useTranslations('landing.spec')

  return (
    <section id="spec">
      <div className="mx-auto max-w-[1200px] px-4 py-16 md:px-6 md:py-20">
        <SectionHeading ns="landing.spec" />

        <div className="mx-auto mt-10 grid max-w-4xl grid-cols-2 gap-x-6 gap-y-10 text-center md:mt-14 md:grid-cols-4">
          {STATS.map(({ value, key }) => (
            <div key={key}>
              <p className="text-ink font-mono text-4xl font-semibold tracking-tight md:text-5xl">
                {value}
              </p>
              <p className="text-ink-muted mt-2 text-xs">{t(key)}</p>
            </div>
          ))}
        </div>

        <div className="border-hairline mx-auto mt-14 max-w-4xl border-t" />

        <ul className="mx-auto mt-8 grid max-w-4xl gap-x-8 gap-y-3 sm:grid-cols-2">
          {ITEMS.map((key) => (
            <li key={key} className="text-ink-muted flex items-center gap-3 text-sm">
              <span className="bg-hairline h-3 w-px flex-shrink-0" aria-hidden="true" />
              {t(key)}
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}
