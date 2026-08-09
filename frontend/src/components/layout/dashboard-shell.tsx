'use client'

// Dashboard 外壳：侧边栏导航 + header。nav 链接用 next/link（路由表 routing-and-guards.md）。
// Admin 入口仅对 superuser 渲染（admin 路由组另有 AdminRoute 守卫兜底）。

import Link from 'next/link'
import { type ReactNode } from 'react'
import { useTranslations } from 'next-intl'
import { Shield } from 'lucide-react'
import { ThemeToggle } from '@/components/ui/theme-toggle'
import { LanguageSwitcher } from '@/components/ui/language-switcher'
import { useAuthStore } from '@/stores/auth'
import { UserMenu } from './user-menu'
import { BrandLogo } from '@/components/brand/brand-logo'

export function DashboardShell({ children }: { children: ReactNode }) {
  const t = useTranslations('navigation')
  const isSuperuser = useAuthStore((s) => s.user?.is_superuser === true)
  return (
    <div className="bg-canvas min-h-screen">
      <header className="border-hairline bg-surface-1 sticky top-0 z-10 border-b">
        <div className="mx-auto flex h-14 max-w-[1200px] items-center gap-6 px-6">
          <Link href="/dashboard">
            <BrandLogo size={26} />
          </Link>
          <nav className="text-ink-muted flex items-center gap-4 text-sm">
            <Link href="/devices" className="hover:text-ink">
              {t('devices')}
            </Link>
            <Link href="/agents" className="hover:text-ink">
              {t('agent')}
            </Link>
            <Link href="/integrations" className="hover:text-ink">
              {t('integrations')}
            </Link>
            <Link href="/wakes" className="hover:text-ink">
              {t('history')}
            </Link>
            {isSuperuser && (
              <Link
                href="/admin"
                className="text-primary hover:text-primary/80 inline-flex items-center gap-1 font-medium"
              >
                <Shield className="h-3.5 w-3.5" />
                {t('admin')}
              </Link>
            )}
          </nav>
          <div className="ml-auto flex items-center gap-2">
            <LanguageSwitcher />
            <ThemeToggle />
            <UserMenu />
          </div>
        </div>
      </header>
      <main className="mx-auto max-w-[1200px] px-6 py-8">{children}</main>
    </div>
  )
}
