'use client'

// 注册成功后的引导页：提示查收验证邮件 + resend 按钮（60s 倒计时 disable）+ 回登录入口。
// email 来自 query（register 跳转携带），无 email 时兜底空串。
// 倒计时：挂载即开始 60s（注册刚发信，挂载时刻 ≈ 发信时刻，与后端 per-user 60s 冷却对齐）；
// 发送成功后重置 60s；429 时取 max(60, Retry-After)——IP 级限流的真实回补窗口可能远超一分钟，
// 倒计时与文案如实反映（UX 修复：旧版固定文案"请等待一分钟"与真实限制不符）。

import { Suspense, useEffect, useState } from 'react'
import Link from 'next/link'
import { useSearchParams } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { AuthShell } from '@/components/auth/auth-shell'

/** 与后端 RESEND_COOLDOWN_SECS 对齐（email_verification_service.rs）。 */
const COOLDOWN_SECS = 60

/** 秒数 → m:ss 倒计时文案（1:00 / 19:45），locale 无关。 */
function formatCountdown(totalSecs: number): string {
  const m = Math.floor(totalSecs / 60)
  const s = totalSecs % 60
  return `${m}:${String(s).padStart(2, '0')}`
}

function CheckEmailContent() {
  const t = useTranslations('auth')
  const searchParams = useSearchParams()
  const email = searchParams.get('email') ?? ''
  const [sending, setSending] = useState(false)
  const [remaining, setRemaining] = useState(COOLDOWN_SECS)

  // setInterval 而非链式 setTimeout:倒计时只需单调递减,单一 interval 在
  // remaining 翻 0 时清理;重置(发送成功/429)时读到最新值继续走。
  const counting = remaining > 0
  useEffect(() => {
    if (!counting) return
    const id = setInterval(() => setRemaining((r) => (r > 0 ? r - 1 : r)), 1000)
    return () => clearInterval(id)
  }, [counting])

  const onResend = async () => {
    if (!email || remaining > 0 || sending) return
    setSending(true)
    try {
      await api.auth.resendVerification(email)
      toast.success(t('resendSent'))
      setRemaining(COOLDOWN_SECS)
    } catch (err) {
      if (err instanceof ApiError && err.status === 429) {
        // 冷却中（IP 级限流）：倒计时取真实回补窗口与本地冷却的较大者。
        setRemaining(Math.max(err.retryAfterSeconds ?? COOLDOWN_SECS, COOLDOWN_SECS))
        toast.error(t('resendRateLimited'))
      } else {
        // 其他错误统一显示成功（防枚举）。
        toast.success(t('resendSent'))
        setRemaining(COOLDOWN_SECS)
      }
    } finally {
      setSending(false)
    }
  }

  return (
    <AuthShell
      title={t('checkEmail')}
      subtitle={t('checkEmailHint', { email })}
      footer={
        <div className="flex flex-col items-center gap-2">
          <Link href="/login" className="text-ink hover:text-ink-muted font-medium underline">
            {t('backToLogin')}
          </Link>
          {/* 输错邮箱的逃生口：回 register 用正确邮箱重注（旧未验证账号不影响） */}
          <p className="text-xs">
            {t('wrongEmail')}{' '}
            <Link href="/register" className="text-ink-muted hover:text-ink underline">
              {t('useDifferentEmail')}
            </Link>
          </p>
        </div>
      }
    >
      <div className="space-y-4">
        <div className="bg-success/10 text-success flex items-center justify-center rounded-full p-4">
          {/* 信封图标 */}
          <svg
            className="h-7 w-7"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <rect width="20" height="16" x="2" y="4" rx="2" />
            <path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />
          </svg>
        </div>
        <p className="text-ink-subtle text-center text-xs">{t('checkEmailSpam')}</p>
        <button
          type="button"
          onClick={onResend}
          disabled={sending || !email || remaining > 0}
          className="border-hairline text-ink hover:bg-surface-2 h-10 w-full rounded-md border text-sm font-medium transition-colors disabled:opacity-50"
        >
          {sending
            ? t('resending')
            : remaining > 0
              ? t('resendCountdown', { time: formatCountdown(remaining) })
              : t('resendVerification')}
        </button>
      </div>
    </AuthShell>
  )
}

export default function CheckEmailPage() {
  return (
    <Suspense fallback={<div className="flex min-h-screen items-center justify-center">...</div>}>
      <CheckEmailContent />
    </Suspense>
  )
}
