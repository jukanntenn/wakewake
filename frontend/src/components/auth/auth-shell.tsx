'use client'

// Auth 页面外壳：Vercel/Linear 风格的分屏布局。
// 左侧（lg+）：品牌叙事区（深色渐变 + tagline + 特性点）。
// 右侧：表单卡（含 logo + 标题/副标题 + children + 底部交叉链接）。
// 移动端：只渲染右侧表单（带顶部 logo）。

import { type ReactNode } from 'react'
import { useTranslations } from 'next-intl'
import { BrandMark } from '@/components/brand/brand-logo'

interface AuthShellProps {
  title: string
  subtitle?: string
  children: ReactNode
  footer?: ReactNode
}

export function AuthShell({ title, subtitle, children, footer }: AuthShellProps) {
  const t = useTranslations('auth')
  return (
    <div className="bg-canvas flex min-h-screen">
      {/* 品牌叙事区（lg+ 可见）。固定深色背景（不随主题翻转），保持品牌一致性。
          text-canvas 在深色主题下会变成深色 → 这里改用固定浅色确保可读。 */}
      <div
        className="relative hidden w-1/2 flex-col justify-between overflow-hidden p-12 lg:flex"
        style={{ backgroundColor: 'oklch(14.9% 0.024 285.8)' }}
      >
        {/* 装饰：径向光晕 */}
        <div
          className="pointer-events-none absolute inset-0 opacity-60"
          style={{
            background:
              'radial-gradient(circle at 20% 20%, rgba(76,217,100,0.15), transparent 45%), radial-gradient(circle at 80% 70%, rgba(120,140,255,0.12), transparent 50%)',
          }}
        />
        <div className="relative flex items-center gap-2 text-lg font-semibold tracking-tight text-white">
          <BrandMark size={28} />
          WakeWake
        </div>
        <div className="relative space-y-4 text-white">
          <p className="text-3xl leading-tight font-semibold tracking-tight">{t('brandTagline')}</p>
          <ul className="space-y-2 text-sm text-white/70">
            <FeaturePoint>{t('createAccountSubtitle')}</FeaturePoint>
            <FeaturePoint>{t('welcomeBackSubtitle')}</FeaturePoint>
          </ul>
        </div>
        <div className="relative text-xs text-white/40">
          &copy; {new Date().getFullYear()} WakeWake
        </div>
      </div>

      {/* 表单区 */}
      <div className="bg-canvas flex w-full flex-col items-center justify-center px-4 py-12 lg:w-1/2">
        <div className="w-full max-w-sm">
          {/* 移动端 logo */}
          <div className="mb-8 flex justify-center lg:hidden">
            <BrandMark size={36} />
          </div>
          <div className="mb-6 space-y-1">
            <h1 className="text-ink text-2xl font-semibold tracking-tight">{title}</h1>
            {subtitle && <p className="text-ink-muted text-sm">{subtitle}</p>}
          </div>
          {children}
          {footer && <div className="text-ink-muted mt-6 text-center text-sm">{footer}</div>}
        </div>
      </div>
    </div>
  )
}

function FeaturePoint({ children }: { children: ReactNode }) {
  return (
    <li className="flex items-center gap-2">
      <svg
        className="text-success h-4 w-4 flex-shrink-0"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={3}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M20 6 9 17l-5-5" />
      </svg>
      {children}
    </li>
  )
}
