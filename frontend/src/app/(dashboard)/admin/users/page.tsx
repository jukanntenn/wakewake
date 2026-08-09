'use client'

// Admin Users 页面（/admin/users）。AdminRoute 守卫。
// 列全部用户 + disable/enable/重置密码/强制验证邮箱。
// 重置密码用 Dialog 收集新密码；强制验证邮箱用确认 Dialog（破坏性操作的二次确认）。

import { useState } from 'react'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Loader2 } from 'lucide-react'
import { useAuthStore } from '@/stores/auth'
import { ApiError } from '@/lib/api'
import { useAdminUsers, useDisableUser, useEnableUser } from '@/hooks/useAdminUsers'
import { useResetUserPassword, useVerifyUserEmail } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

interface ResetState {
  id: number
  email: string
}
interface VerifyState {
  id: number
  email: string
}

export default function AdminUsersPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const user = useAuthStore((s) => s.user)
  const { data: users, isLoading } = useAdminUsers()
  const disableMut = useDisableUser()
  const enableMut = useEnableUser()
  const resetMut = useResetUserPassword()
  const verifyMut = useVerifyUserEmail()
  const [resetTarget, setResetTarget] = useState<ResetState | null>(null)
  const [verifyTarget, setVerifyTarget] = useState<VerifyState | null>(null)
  const [newPassword, setNewPassword] = useState('')

  const userList = users ?? []

  const onToggle = async (id: number, currentlyActive: boolean) => {
    try {
      if (currentlyActive) {
        await disableMut.mutateAsync(id)
        toast.success(t('disableSuccess'))
      } else {
        await enableMut.mutateAsync(id)
        toast.success(t('enableSuccess'))
      }
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  const openReset = (u: { id: number; email: string }) => {
    setNewPassword('')
    setResetTarget(u)
  }

  const submitReset = async () => {
    if (!resetTarget) return
    if (newPassword.length < 8) {
      toast.error(t('passwordTooShort'))
      return
    }
    try {
      await resetMut.mutateAsync({ id: resetTarget.id, newPassword })
      toast.success(t('resetPasswordSuccess'))
      setResetTarget(null)
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  const submitVerify = async () => {
    if (!verifyTarget) return
    try {
      await verifyMut.mutateAsync(verifyTarget.id)
      toast.success(t('verifyEmailSuccess'))
      setVerifyTarget(null)
    } catch (err) {
      toast.error(tErr((err instanceof ApiError ? err.code : 'INTERNAL_ERROR') as 'SYNCING'))
    }
  }

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('users')}</h1>

      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <h2 className="text-ink font-medium">{t('userManagement')}</h2>
        <p className="text-ink-muted mt-2 text-sm">{t('userManagementDesc')}</p>

        {isLoading ? (
          <div className="text-ink-subtle mt-4 flex h-32 items-center justify-center text-sm">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t('loading')}
          </div>
        ) : userList.length > 0 ? (
          <div className="mt-4 overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-hairline text-ink-muted border-b text-left">
                  <th className="py-2 pr-4 font-medium">ID</th>
                  <th className="py-2 pr-4 font-medium">{t('email')}</th>
                  <th className="py-2 pr-4 font-medium">{t('role')}</th>
                  <th className="py-2 pr-4 font-medium">{t('status')}</th>
                  <th className="py-2 pr-4 font-medium">{t('emailVerified')}</th>
                  <th className="py-2 pr-4 font-medium">{t('created')}</th>
                  <th className="py-2 pr-4 font-medium">{t('actions')}</th>
                </tr>
              </thead>
              <tbody>
                {userList.map((u) => {
                  const isSelf = u.id === user?.id
                  return (
                    <tr key={u.id} className="border-hairline border-b">
                      <td className="text-ink-subtle py-2 pr-4">{u.id}</td>
                      <td className="text-ink py-2 pr-4">
                        {u.email}
                        {isSelf && (
                          <span className="text-ink-subtle ml-1 text-xs">({t('you')})</span>
                        )}
                      </td>
                      <td className="text-ink-muted py-2 pr-4">
                        {u.is_superuser ? t('superuser') : t('user')}
                      </td>
                      <td className="py-2 pr-4">
                        <span
                          className={
                            u.is_active
                              ? 'inline-flex items-center text-green-600 dark:text-green-400'
                              : 'text-ink-subtle inline-flex items-center'
                          }
                        >
                          <span
                            className={
                              'mr-1.5 h-2 w-2 rounded-full ' +
                              (u.is_active ? 'bg-green-500' : 'bg-gray-400')
                            }
                          />
                          {u.is_active ? t('active') : t('disabled')}
                        </span>
                      </td>
                      <td className="py-2 pr-4">
                        {u.email_verified ? (
                          <span className="text-green-600 dark:text-green-400">
                            {t('verified')}
                          </span>
                        ) : (
                          <span className="text-amber-600 dark:text-amber-400">
                            {t('notVerified')}
                          </span>
                        )}
                      </td>
                      <td className="text-ink-subtle py-2 pr-4">
                        {new Date(u.created_at).toLocaleString()}
                      </td>
                      <td className="py-2 pr-4">
                        <div className="flex flex-wrap gap-1.5">
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={isSelf || disableMut.isPending || enableMut.isPending}
                            onClick={() => onToggle(u.id, u.is_active)}
                          >
                            {u.is_active ? t('disable') : t('enable')}
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={isSelf || resetMut.isPending}
                            onClick={() => openReset(u)}
                          >
                            {t('resetPassword')}
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={isSelf || u.email_verified || verifyMut.isPending}
                            onClick={() => setVerifyTarget(u)}
                          >
                            {t('forceVerify')}
                          </Button>
                        </div>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="border-hairline text-ink-subtle mt-4 flex h-32 items-center justify-center rounded-sm border border-dashed text-sm">
            {t('empty')}
          </div>
        )}
      </div>

      {/* 重置密码 Dialog */}
      <Dialog open={resetTarget !== null} onOpenChange={(o) => !o && setResetTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('resetPasswordTitle')}</DialogTitle>
            <DialogDescription>
              {t('resetPasswordDesc', { email: resetTarget?.email ?? '' })}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <label className="text-ink-muted text-sm">{t('newPassword')}</label>
            <Input
              type="password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              placeholder={t('newPasswordPlaceholder')}
              autoFocus
            />
            <p className="text-ink-subtle text-xs">{t('resetPasswordWarning')}</p>
          </div>
          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={() => setResetTarget(null)}>
              {t('cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={submitReset}
              disabled={resetMut.isPending || newPassword.length < 8}
            >
              {resetMut.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                t('resetPassword')
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 强制验证邮箱确认 Dialog */}
      <Dialog open={verifyTarget !== null} onOpenChange={(o) => !o && setVerifyTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('forceVerifyTitle')}</DialogTitle>
            <DialogDescription>
              {t('forceVerifyDesc', { email: verifyTarget?.email ?? '' })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={() => setVerifyTarget(null)}>
              {t('cancel')}
            </Button>
            <Button onClick={submitVerify} disabled={verifyMut.isPending}>
              {verifyMut.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : t('confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
