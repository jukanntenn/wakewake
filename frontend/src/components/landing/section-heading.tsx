'use client'

// Landing section 头部统一模式:caption-mono kicker(§ 前缀)+ display-md 标题。
// 全页四个 section 共用,保证节律一致。

import { useTranslations } from 'next-intl'

interface SectionHeadingProps {
  ns: 'landing.hops' | 'landing.trust' | 'landing.deploy' | 'landing.why'
}

export function SectionHeading({ ns }: SectionHeadingProps) {
  const t = useTranslations(ns)
  return (
    <div className="mx-auto max-w-2xl text-center">
      <p className="text-ink-subtle font-mono text-xs tracking-widest uppercase">§ {t('kicker')}</p>
      <h2 className="text-ink mt-3 text-2xl font-semibold tracking-tight md:text-3xl">
        {t('title')}
      </h2>
    </div>
  )
}
