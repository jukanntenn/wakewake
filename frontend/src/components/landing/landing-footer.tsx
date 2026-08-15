'use client'

// Landing 收尾:一行收束 + 重复主 CTA(一页一动作),然后 hairline 单行 footer。

import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { ArrowUpRight } from 'lucide-react'
import { BrandMark } from '@/components/brand/brand-logo'
import { Button } from '@/components/ui/button'
import { GITHUB_URL } from './landing-header'

export function LandingFooter() {
  const t = useTranslations('landing')

  return (
    <>
      <div className="mx-auto max-w-[1200px] px-4 py-16 text-center md:px-6 md:py-20">
        <p className="text-ink text-xl font-semibold tracking-tight md:text-2xl">
          {t('closing.line')}
        </p>
        <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
          <Button asChild>
            <Link href="/register">{t('hero.ctaPrimary')}</Link>
          </Button>
          <Button variant="outline" asChild>
            <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
              {t('hero.ctaGithub')}
              <ArrowUpRight className="ml-1.5 h-4 w-4" />
            </a>
          </Button>
        </div>
      </div>

      <footer className="border-hairline border-t">
        <div className="text-ink-muted mx-auto flex max-w-[1200px] flex-col items-center justify-between gap-4 px-4 py-8 text-xs sm:flex-row md:px-6">
          <div className="flex items-center gap-2">
            <BrandMark size={18} />
            <span>
              © {new Date().getFullYear()} WakeWake · {t('footer.selfHosted')}
            </span>
          </div>
          <nav className="flex items-center gap-5" aria-label={t('footer.docs')}>
            <a
              href="https://github.com/jukanntenn/wakewake/blob/main/LICENSE"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-ink transition-colors"
            >
              AGPL-3.0
            </a>
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-ink transition-colors"
            >
              GitHub ↗
            </a>
            <a
              href="https://github.com/jukanntenn/wakewake/tree/main/specs"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-ink transition-colors"
            >
              {t('footer.docs')} ↗
            </a>
          </nav>
        </div>
      </footer>
    </>
  )
}
