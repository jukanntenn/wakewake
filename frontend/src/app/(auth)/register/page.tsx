'use client'

import { useState } from 'react'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { AuthShell } from '@/components/auth/auth-shell'

const KNOWN_AUTH_ERRORS = [
  'INVALID_CREDENTIALS',
  'USER_EXISTS',
  'AUTH_REQUIRED',
  'TOKEN_EXPIRED',
  'RATE_LIMITED',
] as const

export default function RegisterPage() {
  const t = useTranslations('auth')
  const tErr = useTranslations('auth.error')
  const router = useRouter()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    try {
      // 注册成功 → 不自动登录，跳 check-email 页引导验证。
      await api.auth.register({ email, password })
      toast.success(t('verificationSent'))
      router.push(`/check-email?email=${encodeURIComponent(email)}`)
    } catch (err) {
      const code = err instanceof ApiError ? err.code : 'INTERNAL_ERROR'
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
      title={t('createAccount')}
      subtitle={t('createAccountSubtitle')}
      footer={
        <>
          {t('hasAccount')}{' '}
          <Link href="/login" className="text-ink hover:text-ink-muted font-medium underline">
            {t('signInLink')}
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
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
          />
        </div>
        <div className="space-y-1.5">
          <label htmlFor="password" className="text-ink-muted text-sm font-medium">
            {t('password')}
          </label>
          <input
            id="password"
            type="password"
            autoComplete="new-password"
            required
            minLength={8}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2"
          />
          <p className="text-ink-subtle text-xs">8+ characters</p>
        </div>
        <button
          type="submit"
          disabled={loading}
          className="bg-primary text-on-primary hover:bg-primary/90 h-10 w-full rounded-md text-sm font-medium transition-colors disabled:opacity-50"
        >
          {loading ? t('signingUp') : t('register')}
        </button>
      </form>
    </AuthShell>
  )
}
