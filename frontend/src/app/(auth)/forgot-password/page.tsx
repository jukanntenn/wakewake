'use client'

// Forgot Password 页面：邮箱 + PoW → POST /auth/password-reset/request（authentication.md §六）。
// PoW：GET /pow/challenge → 暴力 nonce → 提交 challenge + nonce。恒显示成功（防枚举）。

import { useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ApiError } from '@/lib/api'
import { AuthShell } from '@/components/auth/auth-shell'

async function solvePow(challenge: string, difficulty: number): Promise<string> {
  const encoder = new TextEncoder()
  for (let nonce = 0; ; nonce++) {
    const data = encoder.encode(challenge + nonce.toString())
    const hash = await crypto.subtle.digest('SHA-256', data)
    const hex = Array.from(new Uint8Array(hash))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
    if (hex.slice(0, difficulty) === '0'.repeat(difficulty)) {
      return nonce.toString()
    }
    if (nonce % 10000 === 0) {
      await new Promise((r) => setTimeout(r, 0))
    }
  }
}

export default function ForgotPasswordPage() {
  const t = useTranslations('auth')
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [sent, setSent] = useState(false)

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setLoading(true)
    try {
      const challengeResp = await fetch('/api/v1/pow/challenge').then((r) => r.json())
      const nonce = await solvePow(challengeResp.challenge, challengeResp.difficulty)
      await fetch('/api/v1/auth/password-reset/request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, challenge: challengeResp.id, nonce }),
      })
      setSent(true)
      toast.success(t('resetLinkSent'))
    } catch (err) {
      if (err instanceof ApiError && err.code === 'RATE_LIMITED') {
        toast.error(t('error.RATE_LIMITED'))
      } else {
        setSent(true)
        toast.success(t('resetLinkSent')) // 防枚举：错误也显示成功
      }
    } finally {
      setLoading(false)
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
          {/* 意识到自己没账号的用户：提供注册入口 */}
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
