'use client'

import { useState } from 'react'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'
import { AuthShell } from '@/components/auth/auth-shell'

const KNOWN_AUTH_ERRORS = [
  'INVALID_CREDENTIALS',
  'USER_EXISTS',
  'AUTH_REQUIRED',
  'TOKEN_EXPIRED',
  'RATE_LIMITED',
] as const

export default function LoginPage() {
  const t = useTranslations('auth')
  const tErr = useTranslations('auth.error')
  const router = useRouter()
  const setAuth = useAuthStore((s) => s.setAuth)
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    try {
      const resp = await api.auth.login({ email, password })
      setAuth(resp.access_token, resp.user, resp.refresh_token)
      router.replace('/dashboard')
    } catch (err) {
      const code = err instanceof ApiError ? err.code : 'INTERNAL_ERROR'
      // 未验证邮箱 → 引导到 check-email 页（携带 email 供 resend 预填）。
      if (code === 'EMAIL_NOT_VERIFIED') {
        toast.message(t('verificationSent'))
        router.push(`/check-email?email=${encodeURIComponent(email)}`)
        return
      }
      const known = (KNOWN_AUTH_ERRORS as readonly string[]).includes(code)
        ? code
        : 'INVALID_CREDENTIALS'
      toast.error(tErr(known as (typeof KNOWN_AUTH_ERRORS)[number]))
    } finally {
      setLoading(false)
    }
  }

  return (
    <AuthShell
      title={t('welcomeBack')}
      subtitle={t('welcomeBackSubtitle')}
      footer={
        <>
          {t('noAccount')}{' '}
          <Link href="/register" className="text-ink hover:text-ink-muted font-medium underline">
            {t('signUpLink')}
          </Link>
        </>
      }
    >
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
            className="border-hairline bg-surface-1 text-ink focus:border-primary focus-visible:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
          />
        </div>
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <label htmlFor="password" className="text-ink-muted text-sm font-medium">
              {t('password')}
            </label>
            <Link
              href="/forgot-password"
              className="text-ink-muted hover:text-ink text-xs font-medium underline-offset-2 hover:underline"
            >
              {t('forgotPassword')}
            </Link>
          </div>
          <input
            id="password"
            type="password"
            autoComplete="current-password"
            required
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
          />
        </div>
        <button
          type="submit"
          disabled={loading}
          className="bg-primary text-on-primary hover:bg-primary/90 h-10 w-full rounded-md text-sm font-medium transition-colors disabled:opacity-50"
        >
          {loading ? t('signingIn') : t('login')}
        </button>
      </form>
    </AuthShell>
  )
}
