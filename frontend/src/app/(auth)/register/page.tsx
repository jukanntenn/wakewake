'use client'

// 注册页（ui-ux-risk-control §14.3，含 PoW 进度）。
// PoW 移入 Web Worker（§7.1），用户视角进度条（不确定模式，§7.2）。
// 不暴露技术细节：进度文案 "Creating account..."，无百分比无 PoW 字段。
// 维护态主动联动（admin-risk-controls WRFC）：任何维护档位都拦截注册
// （maintenance 中间件对三档均 403），轮询公开的 /health/maintenance，
// 关闭期间禁用表单并显示公告——避免用户算完 PoW 才吃到 toast 错误。

import { useState } from 'react'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { solvePow } from '@/lib/pow'
import { AuthShell } from '@/components/auth/auth-shell'

const KNOWN_AUTH_ERRORS = [
  'INVALID_CREDENTIALS',
  'USER_EXISTS',
  'AUTH_REQUIRED',
  'TOKEN_EXPIRED',
  'RATE_LIMITED',
  'MAINTENANCE_REGISTRATION_CLOSED',
] as const

export default function RegisterPage() {
  const t = useTranslations('auth')
  const tErr = useTranslations('auth.error')
  const router = useRouter()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [powProgress, setPowProgress] = useState(false)

  // 维护横幅同源（公开端点）：enabled 即关闭注册（三档均拦 POST /auth/register）。
  const { data: maintenance } = useQuery({
    queryKey: ['maintenance-status'],
    queryFn: () => api.health.maintenance(),
    refetchInterval: 60_000,
    staleTime: 30_000,
  })
  const registrationClosed = maintenance?.enabled === true

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    setPowProgress(true)
    try {
      // PoW 求解（Worker，§7.1）。用户视角文案 "Creating account..."（§7.2）。
      const pow = await solvePow(
        () => api.auth.powChallenge(),
        () => {},
      )
      setPowProgress(false)
      await api.auth.register({
        email,
        password,
        challenge: pow.challenge,
        nonce: pow.nonce,
      })
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
      setPowProgress(false)
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
      {registrationClosed && (
        <div className="bg-warning/10 border-warning/30 text-ink rounded-md border px-3 py-2.5 text-sm">
          {maintenance?.message?.trim() ? maintenance.message : t('registrationClosed')}
        </div>
      )}
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
            disabled={registrationClosed}
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2 disabled:opacity-50"
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
            disabled={registrationClosed}
            className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-10 w-full rounded-md border px-3 text-sm transition-colors outline-none focus:ring-2 disabled:opacity-50"
          />
          <p className="text-ink-subtle text-xs">{t('passwordHint')}</p>
        </div>

        {/* PoW 进度条（不确定模式，§7.2） */}
        {powProgress && (
          <div className="space-y-1">
            <div className="bg-surface-2 h-1.5 w-full overflow-hidden rounded-full">
              <div className="bg-primary h-full w-1/3 animate-pulse rounded-full" />
            </div>
            <p className="text-ink-muted text-center text-xs">{t('creatingAccount')}</p>
          </div>
        )}

        <button
          type="submit"
          disabled={loading || registrationClosed}
          className="bg-primary text-on-primary hover:bg-primary/90 h-10 w-full rounded-md text-sm font-medium transition-colors disabled:opacity-50"
        >
          {loading ? t('signingUp') : t('register')}
        </button>
      </form>
    </AuthShell>
  )
}
