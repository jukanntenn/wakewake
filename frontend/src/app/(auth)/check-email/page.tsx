'use client'

// 注册成功后的引导页：提示查收验证邮件 + resend 按钮 + 回登录入口。
// email 来自 query（register 跳转携带），无 email 时兜底空串。

import { useState, Suspense } from 'react'
import Link from 'next/link'
import { useSearchParams } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { AuthShell } from '@/components/auth/auth-shell'

function CheckEmailContent() {
  const t = useTranslations('auth')
  const searchParams = useSearchParams()
  const email = searchParams.get('email') ?? ''
  const [sending, setSending] = useState(false)

  const onResend = async () => {
    if (!email) return
    setSending(true)
    try {
      await api.auth.resendVerification(email)
      toast.success(t('resendSent'))
    } catch (err) {
      // 429 → 冷却中；其他错误统一显示成功（防枚举）。
      if (err instanceof ApiError && err.status === 429) {
        toast.error(t('resendCooldown'))
      } else {
        toast.success(t('resendSent'))
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
          disabled={sending || !email}
          className="border-hairline text-ink hover:bg-surface-2 h-10 w-full rounded-md border text-sm font-medium transition-colors disabled:opacity-50"
        >
          {sending ? t('resending') : t('resendVerification')}
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
