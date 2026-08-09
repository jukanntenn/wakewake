'use client'

// Reset Password 页面：从 URL ?token=... 读取 → 输入新密码 → POST /auth/password-reset/confirm。
// 成功后用返回的新 token 对自动登录。包含确认密码字段防止输入错误。

import { useState, Suspense } from 'react'
import Link from 'next/link'
import { useRouter, useSearchParams } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'
import { AuthShell } from '@/components/auth/auth-shell'

function ResetPasswordContent() {
  const t = useTranslations('auth')
  const tVal = useTranslations('validation')
  const router = useRouter()
  const searchParams = useSearchParams()
  const setAuth = useAuthStore((s) => s.setAuth)
  const token = searchParams.get('token') ?? ''
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [loading, setLoading] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!token) {
      toast.error(t('error.INVALID_TOKEN'))
      return
    }
    if (newPassword !== confirmPassword) {
      toast.error(tVal('invalid_format', { field: t('confirmPassword') }))
      return
    }
    setLoading(true)
    try {
      const resp = await fetch('/api/v1/auth/password-reset/confirm', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token, new_password: newPassword }),
      })
      if (!resp.ok) {
        throw new ApiError('INVALID_TOKEN', 'Reset failed', resp.status)
      }
      const data = await resp.json()
      const userResp = await api.user.me()
      setAuth(data.access_token, userResp, data.refresh_token)
      toast.success(t('passwordReset'))
      router.push('/dashboard')
    } catch {
      toast.error(t('error.INVALID_TOKEN'))
    } finally {
      setLoading(false)
    }
  }

  if (!token) {
    return (
      <AuthShell title={t('resetPassword')}>
        <div className="space-y-4 text-center">
          <div className="bg-destructive/10 text-destructive flex items-center justify-center rounded-full p-4">
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
          <p className="text-ink-muted text-sm">{t('error.INVALID_TOKEN')}</p>
          <Link
            href="/forgot-password"
            className="text-ink hover:text-ink-muted text-sm font-medium underline"
          >
            {t('forgotPassword')}
          </Link>
        </div>
      </AuthShell>
    )
  }

  return (
    <AuthShell
      title={t('resetPassword')}
      footer={
        <Link href="/login" className="text-ink hover:text-ink-muted font-medium underline">
          {t('backToLogin')}
        </Link>
      }
    >
      <form onSubmit={onSubmit} className="space-y-4">
        <div className="space-y-1.5">
          <label htmlFor="newPassword" className="text-ink-muted text-sm font-medium">
            {t('newPassword')}
          </label>
          <input
            id="newPassword"
            type="password"
            autoComplete="new-password"
            required
            minLength={8}
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
          />
        </div>
        <div className="space-y-1.5">
          <label htmlFor="confirmPassword" className="text-ink-muted text-sm font-medium">
            {t('confirmPassword')}
          </label>
          <input
            id="confirmPassword"
            type="password"
            autoComplete="new-password"
            required
            minLength={8}
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            className={`border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2 ${
              confirmPassword && confirmPassword !== newPassword ? 'border-destructive' : ''
            }`}
          />
          {confirmPassword && confirmPassword !== newPassword && (
            <span className="text-destructive text-xs">
              {tVal('invalid_format', { field: t('confirmPassword') })}
            </span>
          )}
        </div>
        <button
          type="submit"
          disabled={loading}
          className="bg-primary text-on-primary hover:bg-primary/90 h-10 w-full rounded-md text-sm font-medium transition-colors disabled:opacity-50"
        >
          {loading ? t('sending') : t('resetPassword')}
        </button>
      </form>
    </AuthShell>
  )
}

export default function ResetPasswordPage() {
  return (
    <Suspense fallback={<div className="flex min-h-screen items-center justify-center">...</div>}>
      <ResetPasswordContent />
    </Suspense>
  )
}
