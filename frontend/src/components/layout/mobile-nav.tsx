'use client'

// MobileNav：移动端汉堡全屏导航（ui-ux-risk-control §2.3）。
// 语言/主题不收进（已在顶栏工具条直达）。Settings/Logout 收进。
// children 是 trigger 元素（汉堡按钮），点击打开全屏导航。

import { type ReactNode, useState, cloneElement, isValidElement } from 'react'
import { useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { useAuthStore } from '@/stores/auth'
import { api } from '@/lib/api'
import { toast } from 'sonner'

const NAV_ITEMS = [
  { href: '/devices', key: 'devices' as const },
  { href: '/agents', key: 'agent' as const },
  { href: '/integrations', key: 'integrations' as const },
  { href: '/wakes', key: 'history' as const },
]

export function MobileNav({ children }: { children: ReactNode }) {
  const t = useTranslations('navigation')
  const tCommon = useTranslations('common')
  const router = useRouter()
  const [open, setOpen] = useState(false)
  const isSuperuser = useAuthStore((s) => s.user?.is_superuser === true)
  const refreshToken = useAuthStore((s) => s.refreshToken)
  const logout = useAuthStore((s) => s.logout)

  const navigate = (href: string) => {
    setOpen(false)
    router.push(href)
  }

  const handleLogout = async () => {
    setOpen(false)
    if (refreshToken) {
      try {
        await api.user.logout(refreshToken)
      } catch {
        // 忽略
      }
    }
    logout()
    router.push('/login')
    toast.success(tCommon('logout'))
  }

  // 把 trigger 的 onClick 绑定到 setOpen(true)
  const trigger = isValidElement(children)
    ? cloneElement(children as React.ReactElement<{ onClick?: () => void }>, {
        onClick: () => setOpen(true),
      })
    : children

  return (
    <>
      {trigger}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent
          showCloseButton
          className="bg-canvas fixed inset-0 top-auto h-full max-h-full max-w-none -translate-x-0 -translate-y-0 rounded-none border-0 p-6"
        >
          <DialogTitle className="sr-only">{t('menu')}</DialogTitle>
          <div className="flex h-full flex-col">
            <nav className="space-y-2">
              {NAV_ITEMS.map((item) => (
                <button
                  key={item.href}
                  onClick={() => navigate(item.href)}
                  className="text-ink hover:bg-surface-2 block w-full rounded-md px-3 py-2 text-left text-lg font-medium"
                >
                  {t(item.key)}
                </button>
              ))}
              {isSuperuser && (
                <>
                  <hr className="border-hairline my-2" />
                  <button
                    onClick={() => navigate('/admin')}
                    className="text-primary hover:bg-surface-2 block w-full rounded-md px-3 py-2 text-left text-lg font-medium"
                  >
                    {t('admin')}
                  </button>
                </>
              )}
            </nav>
            <div className="border-hairline mt-auto space-y-2 border-t pt-4">
              <button
                onClick={() => navigate('/settings')}
                className="text-ink hover:bg-surface-2 block w-full rounded-md px-3 py-2 text-left"
              >
                {t('settings')}
              </button>
              <button
                onClick={handleLogout}
                className="text-ink-muted hover:bg-surface-2 block w-full rounded-md px-3 py-2 text-left"
              >
                {tCommon('logout')}
              </button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
