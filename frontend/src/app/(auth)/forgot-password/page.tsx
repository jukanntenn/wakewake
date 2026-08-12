'use client'

// Forgot Password 页面（ui-ux-risk-control §14.4，PoW 移入 Worker）。
// PoW：GET /pow/challenge → Worker 求 nonce → POST /auth/password-reset/request。
// 恒显示成功（防枚举）。用户视角进度条（不确定模式，文案 "Sending reset link..."）。

import { useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { solvePow } from '@/lib/pow'
import { AuthShell } from '@/components/auth/auth-shell'

export default function ForgotPasswordPage() {
  const t = useTranslations('auth')
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [powProgress, setPowProgress] = useState(false)
  const [sent, setSent] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    setPowProgress(true)
    try {
      const pow = await solvePow(
        () => api.auth.powChallenge(),
        () => {},
      )
      setPowProgress(false)
      await api.auth.requestPasswordReset({ email, challenge: pow.challenge, nonce: pow.nonce })
      setSent(true)
      toast.success(t('resetLinkSent'))
    } catch (err) {
      if (err instanceof ApiError && err.code === 'RATE_LIMITED') {
        toast.error(t('error.RATE_LIMITED'))
      } else {
        // 防枚举：其他错误也显示成功
        setSent(true)
        toast.success(t('resetLinkSent'))
      }
    } finally {
      setLoading(false)
      setPowProgress(false)
    }
  }

  return (
    <AuthShell
      title={t('forgotPassword')}
      subtitle={sent ? t('resetLinkSent') : t('resetEmailHint')}
      footer={
        <div className="flex flex-col items-center gap-2">
          <Link href="/login" className="text-ink hover:text-ink-muted font-medium underline">
            {t('backToLogin')}
          </Link>
          <p className="text-xs">
            {t('noAccount')}{' '}
            <Link href="/register" className="text-ink-muted hover:text-ink underline">
              {t('signUpLink')}
            </Link>
          </p>
        </div>
      }
    >
      {sent ? (
        <div className="bg-success/10 text-success flex items-center justify-center rounded-full p-4">
          <svg
            className="h-7 w-7"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M22 2 11 13" />
            <path d="M22 2 15 22l-4-9-9-4Z" />
          </svg>
        </div>
      ) : (
        <form onSubmit={onSubmit} className="space-y-4">
          <div className="space-y-1.5">
            <label htmlFor="email" className="text-ink-muted text-sm font-medium">
              {t('email')}
            </label>
            <input
              id="email"
              type="email"
              autoComplete="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
            />
          </div>

          {/* PoW 进度条（不确定模式，§14.4） */}
          {powProgress && (
            <div className="space-y-1">
              <div className="bg-surface-2 h-1.5 w-full overflow-hidden rounded-full">
                <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
              </div>
              <p className="text-ink-muted text-center text-xs">{t('sendingResetLink')}</p>
            </div>
          )}

          <button
            type="submit"
            disabled={loading}
            className="bg-primary text-on-primary hover:bg-primary/90 h-10 w-full rounded-md text-sm font-medium transition-colors disabled:opacity-50"
          >
            {loading ? t('sending') : t('sendResetLink')}
          </button>
        </form>
      )}
    </AuthShell>
  )
}
