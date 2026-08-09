'use client'

import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { api } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'

export function UserMenu() {
  const router = useRouter()
  const t = useTranslations('common')
  const { user, refreshToken, logout } = useAuthStore()

  const onLogout = async () => {
    if (refreshToken) {
      try {
        await api.user.logout(refreshToken)
      } catch {
        // best-effort
      }
    }
    logout()
    router.replace('/login')
  }

  if (!user) return null
  return (
    <button
      onClick={onLogout}
      className="text-ink-muted hover:bg-surface-2 hover:text-ink rounded-sm px-3 py-1.5 text-sm"
    >
      {t('logout')}
    </button>
  )
}
