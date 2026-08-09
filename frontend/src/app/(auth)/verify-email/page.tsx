'use client'

// 邮箱验证落地页：从 ?token= 读取 → POST /auth/verify-email → 成功自动登录跳 dashboard。
// 三态：verifying / success / failed。无 token 直接 failed。

import { useEffect, useState, Suspense } from 'react'
import Link from 'next/link'
import { useRouter, useSearchParams } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { api } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'
import { AuthShell } from '@/components/auth/auth-shell'

type Phase = 'verifying' | 'success' | 'failed'

function VerifyEmailContent() {
  const t = useTranslations('auth')
  const router = useRouter()
  const searchParams = useSearchParams()
  const token = searchParams.get('token') ?? ''
  const setAuth = useAuthStore((s) => s.setAuth)
  // 初始态：无 token 直接 failed；有 token 进入 verifying 等待异步结果。
  const [phase, setPhase] = useState<Phase>(token ? 'verifying' : 'failed')

  useEffect(() => {
    if (!token) return
    let cancelled = false
    ;(async () => {
      try {
        const resp = await api.auth.verifyEmail(token)
        if (cancelled) return
        setAuth(resp.access_token, resp.user, resp.refresh_token)
        setPhase('success')
        setTimeout(() => router.replace('/dashboard'), 1500)
      } catch {
        if (!cancelled) setPhase('failed')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [token, setAuth, router])

  return (
    <AuthShell title={t('verifyEmailTitle')}>
      <div className="flex flex-col items-center gap-4 py-4 text-center">
        {phase === 'verifying' && (
          <>
            <Spinner />
            <p className="text-ink-muted text-sm">{t('verifying')}</p>
          </>
        )}
        {phase === 'success' && (
          <>
            <SuccessIcon />
            <p className="text-ink text-sm">{t('verifySuccess')}</p>
          </>
        )}
        {phase === 'failed' && (
          <>
            <ErrorIcon />
            <p className="text-ink font-medium">{t('verifyFailed')}</p>
            <p className="text-ink-muted text-sm">{t('verifyFailedHint')}</p>
            <Link
              href="/login"
              className="bg-primary text-on-primary hover:bg-primary/90 mt-2 h-10 rounded-md px-6 text-sm font-medium transition-colors"
            >
              {t('backToLogin')}
            </Link>
          </>
        )}
      </div>
    </AuthShell>
  )
}

function Spinner() {
  return (
    <svg className="text-ink-muted h-8 w-8 animate-spin" viewBox="0 0 24 24" fill="none">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z"
      />
    </svg>
  )
}

function SuccessIcon() {
  return (
    <div className="bg-success/15 text-success flex h-14 w-14 items-center justify-center rounded-full">
      <svg
        className="h-7 w-7"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={3}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M20 6 9 17l-5-5" />
      </svg>
    </div>
  )
}

function ErrorIcon() {
  return (
    <div className="bg-destructive/10 text-destructive flex h-14 w-14 items-center justify-center rounded-full">
      <svg
        className="h-7 w-7"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
    </div>
  )
}

export default function VerifyEmailPage() {
  return (
    <Suspense fallback={<div className="flex min-h-screen items-center justify-center">...</div>}>
      <VerifyEmailContent />
    </Suspense>
  )
}
