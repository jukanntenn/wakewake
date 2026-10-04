'use client'

// UserMenu：账户区（ui-ux-risk-control §2.2⑤/§12.5）。
// 头像按钮（实心方块 + email 首字母反白）→ Menu 下拉（Settings/Logout）。
// 仅放账户操作（语言/主题已在工具条直达）。

import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { useQueryClient } from '@tanstack/react-query'
import { LogOut, Settings } from 'lucide-react'
import { Menu } from '@/components/ui/menu'
import { api } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'
import { toast } from 'sonner'

export function UserMenu() {
  const router = useRouter()
  const tCommon = useTranslations('common')
  const tNav = useTranslations('navigation')
  const queryClient = useQueryClient()
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
    // 清空 server-state 缓存，防止下一位登录用户看到上个账户的设备/集成等数据。
    queryClient.clear()
    router.replace('/login')
    toast.success(tCommon('logout'))
  }

  if (!user) return null

  const initial = user.email[0]?.toUpperCase() || '?'

  return (
    <Menu>
      <Menu.Trigger
        aria-label={tCommon('menu')}
        className="bg-primary text-on-primary flex h-8 w-8 items-center justify-center rounded-md text-sm font-medium"
      >
        {initial}
      </Menu.Trigger>
      <Menu.Popup>
        <div className="hover:bg-surface-2 flex items-center gap-2 rounded-sm px-2 py-1.5">
          <span className="bg-primary text-on-primary flex h-8 w-8 items-center justify-center rounded-md text-sm font-medium">
            {initial}
          </span>
          <div className="min-w-0">
            <p className="text-ink truncate text-sm font-medium">{user.email}</p>
            {user.is_superuser && <p className="text-primary text-xs">{tNav('admin')}</p>}
          </div>
        </div>
        <Menu.Separator />
        <Menu.Item onClick={() => router.push('/settings')}>
          <Settings className="h-4 w-4" />
          {tNav('settings')}
        </Menu.Item>
        <Menu.Separator />
        <Menu.Item onClick={onLogout} variant="destructive">
          <LogOut className="h-4 w-4" />
          {tCommon('logout')}
        </Menu.Item>
      </Menu.Popup>
    </Menu>
  )
}
