'use client'

// 全局导航壳（ui-ux-risk-control §2/§12.3）。
// 5 分区顶栏：品牌 / 主导航 / 状态胶囊 / 工具条 / 账户。
// active 态：usePathname 判断，当前页加 text-ink + 底部 2px border-primary。
// 移动端主导航收进汉堡全屏菜单（语言/主题不收进，已在工具条直达）。
// 维护横幅紧贴顶栏下方（registration_disabled/readonly 模式展示）。

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { type ReactNode } from 'react'
import { useTranslations } from 'next-intl'
import { Menu as MenuIcon, Shield } from 'lucide-react'
import { cn } from '@/lib/utils'
import { ThemeToggle } from '@/components/ui/theme-toggle'
import { LanguageSwitcher } from '@/components/ui/language-switcher'
import { useAuthStore } from '@/stores/auth'
import { UserMenu } from './user-menu'
import { MobileNav } from './mobile-nav'
import { StatusPill } from './status-pill'
import { MaintenanceBanner } from './maintenance-banner'
import { BrandLogo } from '@/components/brand/brand-logo'

const NAV_ITEMS = [
  { href: '/devices', key: 'devices' as const },
  { href: '/agents', key: 'agent' as const },
  { href: '/integrations', key: 'integrations' as const },
  { href: '/wakes', key: 'history' as const },
]

export function DashboardShell({ children }: { children: ReactNode }) {
  const t = useTranslations('navigation')
  const pathname = usePathname()
  const isSuperuser = useAuthStore((s) => s.user?.is_superuser === true)

  const isActive = (href: string) => pathname === href || pathname.startsWith(href + '/')

  return (
    <div className="bg-canvas min-h-screen">
      <header className="border-hairline bg-surface-1 sticky top-0 z-30 border-b">
        <div className="mx-auto flex h-14 max-w-[1200px] items-center gap-6 px-4 md:px-6">
          {/* ① 移动端汉堡 */}
          <MobileNav>
            <button
              className="hover:bg-surface-2 -ml-2 rounded-md p-2 md:hidden"
              aria-label={t('menu')}
            >
              <MenuIcon className="h-5 w-5" />
            </button>
          </MobileNav>

          {/* ① 品牌 */}
          <Link href="/devices" className="flex shrink-0 items-center gap-2">
            <BrandLogo size={26} />
          </Link>

          {/* ② 主导航（桌面） */}
          <nav className="hidden items-center gap-4 text-sm md:flex">
            {NAV_ITEMS.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  '-mb-[1px] border-b-2 pb-2 transition-colors',
                  isActive(item.href)
                    ? 'text-ink border-primary font-medium'
                    : 'text-ink-muted hover:text-ink border-transparent',
                )}
              >
                {t(item.key)}
              </Link>
            ))}
            {isSuperuser && (
              <Link
                href="/admin"
                className={cn(
                  '-mb-[1px] inline-flex items-center gap-1 border-b-2 pb-2 font-medium transition-colors',
                  isActive('/admin')
                    ? 'text-ink border-primary'
                    : 'text-primary hover:text-primary/80 border-transparent',
                )}
              >
                <Shield className="h-3.5 w-3.5" />
                {t('admin')}
              </Link>
            )}
          </nav>

          {/* ⑤ 账户区（推到最右） */}
          <div className="ml-auto flex items-center gap-2">
            {/* ③ 状态胶囊 */}
            <StatusPill />
            {/* ④ 工具条 */}
            <LanguageSwitcher />
            <ThemeToggle />
            <UserMenu />
          </div>
        </div>
      </header>

      {/* 维护模式横幅（§2.4） */}
      <MaintenanceBanner />

      <main className="mx-auto max-w-[1200px] px-4 py-8 md:px-6">{children}</main>
    </div>
  )
}
