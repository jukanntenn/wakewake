'use client'

// Settings 页面：改密 + 语言偏好（routing-and-guards.md route table：用户设置）。
// 改密：POST /me/password（吊销所有 refresh，强制重登）。
// 语言：localStorage 切换（LocaleProvider.setLocale）。

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { useLocaleContext } from '@/components/providers/LocaleProvider'
import { availableLocales, type Locale } from '@/i18n/constants'

export default function SettingsPage() {
  const t = useTranslations('settings')
  const ta = useTranslations('auth')
  const tErr = useTranslations('auth.error')
  const tVal = useTranslations('validation')
  const router = useRouter()
  const { locale, setLocale } = useLocaleContext()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [loading, setLoading] = useState(false)

  const onChangePassword = async (e: React.FormEvent) => {
    e.preventDefault()
    if (newPassword !== confirmPassword) {
      toast.error(tVal('invalid_format', { field: t('confirmPassword') }))
      return
    }
    setLoading(true)
    try {
      await api.user.changePassword({
        current_password: currentPassword,
        new_password: newPassword,
      })
      toast.success(t('passwordChanged'))
      // 改密吊销所有 refresh → 强制重登
      router.push('/login')
    } catch (err) {
      const code = err instanceof ApiError ? err.code : 'INVALID_CREDENTIALS'
      toast.error(tErr(code as 'INVALID_CREDENTIALS'))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>

      {/* 改密 */}
      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <h2 className="text-ink font-medium">{t('changePassword')}</h2>
        <form onSubmit={onChangePassword} className="mt-4 space-y-3">
          <div className="space-y-1">
            <label className="text-ink-muted text-sm">{ta('password')}</label>
            <input
              type="password"
              required
              minLength={8}
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              placeholder={t('currentPassword')}
              className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none"
            />
          </div>
          <div className="space-y-1">
            <label className="text-ink-muted text-sm">{t('newPassword')}</label>
            <input
              type="password"
              required
              minLength={8}
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              placeholder={t('newPassword')}
              className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none"
            />
          </div>
          <div className="space-y-1">
            <label className="text-ink-muted text-sm">{t('confirmPassword')}</label>
            <input
              type="password"
              required
              minLength={8}
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={t('confirmPassword')}
              className={`border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none ${
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
            className="bg-primary text-on-primary rounded-sm px-4 py-2 text-sm font-medium disabled:opacity-50"
          >
            {loading ? t('saving') : t('changePassword')}
          </button>
        </form>
      </div>

      {/* 语言偏好 */}
      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <h2 className="text-ink font-medium">{t('language')}</h2>
        <div className="mt-3 flex gap-2">
          {availableLocales.map((l) => (
            <button
              key={l}
              onClick={() => setLocale(l as Locale)}
              className={`rounded-sm px-3 py-1.5 text-sm ${
                locale === l
                  ? 'bg-primary text-on-primary'
                  : 'border-hairline text-ink-muted hover:bg-surface-2 border'
              }`}
            >
              {l.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
