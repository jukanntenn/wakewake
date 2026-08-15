'use client'

// Landing header：吸顶,滚动后 canvas/80 + blur + hairline(Vercel 式演变)。
// 复用 BrandLogo / LanguageSwitcher / ThemeToggle,与 dashboard 同族。
// 移动端锚点收下滑面板(仅三个锚点,不需要 dashboard 的全屏 Dialog)。

import { useEffect, useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { ArrowUpRight, Menu, X } from 'lucide-react'
import { BrandLogo } from '@/components/brand/brand-logo'
import { LanguageSwitcher } from '@/components/ui/language-switcher'
import { ThemeToggle } from '@/components/ui/theme-toggle'
import { Button } from '@/components/ui/button'

export const GITHUB_URL = 'https://github.com/jukanntenn/wakewake'

const ANCHORS = [
  { href: '#how-it-works', key: 'howItWorks' as const },
  { href: '#security', key: 'security' as const },
  { href: '#deploy', key: 'deploy' as const },
]

export function LandingHeader() {
  const t = useTranslations('landing.nav')
  const [scrolled, setScrolled] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8)
    onScroll()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  return (
    <header
      className={`sticky top-0 z-30 border-b transition-colors ${
        scrolled ? 'bg-canvas/80 border-hairline backdrop-blur-sm' : 'border-transparent'
      }`}
    >
      <div className="mx-auto flex h-14 max-w-[1200px] items-center gap-6 px-4 md:px-6">
        <Link href="/" aria-label="WakeWake">
          <BrandLogo size={26} />
        </Link>

        <nav className="hidden items-center gap-5 md:flex" aria-label={t('menu')}>
          {ANCHORS.map(({ href, key }) => (
            <a
              key={href}
              href={href}
              className="text-ink-muted hover:text-ink text-sm transition-colors"
            >
              {t(key)}
            </a>
          ))}
        </nav>

        <div className="ml-auto flex items-center gap-2">
          <LanguageSwitcher />
          <ThemeToggle />
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="text-ink-muted hover:bg-surface-2 hover:text-ink hidden items-center gap-1 rounded-md px-2 py-1.5 text-sm transition-colors sm:flex"
          >
            GitHub
            <ArrowUpRight className="h-3.5 w-3.5" />
          </a>
          <Button variant="outline" size="sm" asChild>
            <Link href="/login">{t('signIn')}</Link>
          </Button>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            aria-expanded={menuOpen}
            aria-label={t('menu')}
            className="hover:bg-surface-2 -mr-2 rounded-md p-2 md:hidden"
          >
            {menuOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
          </button>
        </div>
      </div>

      {menuOpen && (
        <nav
          className="border-hairline bg-canvas border-t px-4 py-3 md:hidden"
          aria-label={t('menu')}
        >
          <div className="space-y-1">
            {ANCHORS.map(({ href, key }) => (
              <a
                key={href}
                href={href}
                onClick={() => setMenuOpen(false)}
                className="text-ink hover:bg-surface-2 block rounded-md px-3 py-2 text-sm font-medium"
              >
                {t(key)}
              </a>
            ))}
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="text-ink-muted hover:bg-surface-2 flex items-center gap-1 rounded-md px-3 py-2 text-sm font-medium"
            >
              GitHub
              <ArrowUpRight className="h-3.5 w-3.5" />
            </a>
          </div>
        </nav>
      )}
    </header>
  )
}
