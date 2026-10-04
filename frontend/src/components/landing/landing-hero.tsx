'use client'

// Landing hero:居中主张 + 双 CTA + mono 命令锚点链 + 拓扑图。
// 拓扑后置一层极淡的 success 径向辉光(AuthShell 品牌面板的配额先例)。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { ArrowUpRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { TopologyDiagram } from './topology-diagram'
import { GITHUB_URL } from './landing-header'

export function LandingHero() {
  const t = useTranslations('landing.hero')

  return (
    <section className="relative overflow-hidden pt-16 pb-16 md:pt-24 md:pb-20">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-[480px] opacity-70"
        style={{
          background:
            'radial-gradient(ellipse 40% 50% at 50% 30%, rgba(80,227,194,0.07), transparent 70%)',
        }}
      />
      <div className="relative mx-auto max-w-[1200px] px-4 md:px-6">
        <div className="mx-auto max-w-2xl text-center">
          <h1 className="text-4xl leading-tight font-semibold tracking-tighter md:text-5xl">
            {t('title')}
          </h1>
          <p className="text-ink-muted mt-4 text-lg leading-relaxed">{t('subtitle')}</p>
          <p className="text-ink-subtle mt-3 font-mono text-xs">{t('facts')}</p>

          <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
            <Button size="lg" asChild>
              <Link href="/register">{t('ctaPrimary')}</Link>
            </Button>
            <Button variant="outline" size="lg" asChild>
              <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
                {t('ctaGithub')}
                <ArrowUpRight className="ml-1.5 h-4 w-4" />
              </a>
            </Button>
          </div>
          <a
            href="#deploy"
            className="text-ink-muted hover:text-ink mt-4 inline-block font-mono text-xs underline-offset-4 transition-colors hover:underline"
          >
            $ docker compose up -d ↓
          </a>
        </div>

        <div className="mt-14 md:mt-20">
          <TopologyDiagram />
        </div>
      </div>
    </section>
  )
}
