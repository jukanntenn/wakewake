'use client'

// Admin Users 页（ui-ux-risk-control §9.4）。
// 调查型列表：DataTable + 搜索 + Status/Role 过滤 + 分页。
// 操作：Disable（L3 ConfirmDialog 含封禁原因 input）/ Reset password（L3 含密码 input）/ Enable（L1）。
// 封禁原因内联展示。admin 自我保护（自己的行操作禁用）。

import { useMemo, useState } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, MoreVertical } from 'lucide-react'
import { type AdminUser, ApiError } from '@/lib/api'
import { useAuthStore } from '@/stores/auth'
import { useAdminUsers, useDisableUser, useEnableUser } from '@/hooks/useAdminUsers'
import { useResetUserPassword } from '@/hooks/useAdmin'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Menu } from '@/components/ui/menu'
import { DataTable, type Column } from '@/components/ui/data-table'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { RelativeTime } from '@/components/ui/relative-time'

export default function AdminUsersPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const router = useRouter()
  const searchParams = useSearchParams()
  const currentUser = useAuthStore((s) => s.user)

  // URL query 同步（§11.B.4）：搜索词、过滤、页码同步到 URL。
  const q = searchParams.get('q') ?? ''
  const statusFilter = searchParams.get('status') ?? 'all' // all/active/disabled
  const roleFilter = searchParams.get('role') ?? 'all' // all/user/admin
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? '10')

  const is_active = statusFilter === 'all' ? undefined : statusFilter === 'active'
  const { data: resp, isLoading } = useAdminUsers({
    is_active,
    q: q || undefined,
    page,
    page_size: pageSize,
  })
  const total = resp?.total ?? 0

  const disableMut = useDisableUser()
  const enableMut = useEnableUser()
  const resetMut = useResetUserPassword()

  const [disableTarget, setDisableTarget] = useState<AdminUser | null>(null)
  const [disableReason, setDisableReason] = useState('')
  const [resetTarget, setResetTarget] = useState<AdminUser | null>(null)
  const [newPassword, setNewPassword] = useState('')

  // 客户端过滤：role（后端不支持 role 过滤，前端筛）
  const filteredUsers = useMemo(() => {
    const items = resp?.items ?? []
    if (roleFilter === 'all') return items
    return items.filter((u) => (roleFilter === 'admin' ? u.is_superuser : !u.is_superuser))
  }, [resp?.items, roleFilter])

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/users?${params.toString()}`)
  }

  const onDisable = async () => {
    if (!disableTarget) return
    await disableMut.mutateAsync({ id: disableTarget.id, reason: disableReason || undefined })
    toast.success(t('disableSuccess'))
    setDisableTarget(null)
    setDisableReason('')
  }

  const onResetPassword = async () => {
    if (!resetTarget) return
    if (newPassword.length < 8) {
      throw new Error(t('passwordTooShort'))
    }
    await resetMut.mutateAsync({ id: resetTarget.id, newPassword })
    toast.success(t('resetPasswordSuccess'))
    setResetTarget(null)
    setNewPassword('')
  }

  const onEnable = async (user: AdminUser) => {
    try {
      await enableMut.mutateAsync(user.id)
      toast.success(t('enableSuccess'))
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const columns: Column<AdminUser>[] = [
    { key: 'email', label: t('email' as never) ?? 'User' },
    { key: 'status', label: t('status') },
    { key: 'role', label: t('role') },
    { key: 'last_login', label: t('lastLogin') },
    { key: 'actions', label: t('actions' as never) ?? '' },
  ]

  const renderCell = (user: AdminUser, key: string) => {
    const isSelf = currentUser?.id === user.id
    if (key === 'email') {
      return (
        <div className={user.is_active ? '' : 'opacity-60'}>
          <span className="text-ink font-medium">{user.email}</span>
          {!user.email_verified && <span className="text-ink-subtle ml-1 text-xs">✕</span>}
        </div>
      )
    }
    if (key === 'status') {
      return (
        <span className="inline-flex items-center gap-1.5 text-sm">
          <StatusIndicator color={user.is_active ? 'success' : 'neutral'} />
          <span className="text-ink-muted">
            {user.is_active ? (t('statusActive' as never) ?? t('active')) : t('disabled')}
          </span>
        </span>
      )
    }
    if (key === 'role') {
      return user.is_superuser ? (
        <span className="text-primary text-sm">★ {t('superuser')}</span>
      ) : (
        <span className="text-ink-muted text-sm">{t('user' as never) ?? 'User'}</span>
      )
    }
    if (key === 'last_login') {
      return (
        <span className="text-ink-muted text-sm">
          <RelativeTime date={user.last_login} fallback={t('never')} />
        </span>
      )
    }
    if (key === 'actions') {
      if (isSelf) {
        return <span className="text-ink-subtle text-xs">({t('thisIsYou' as never) ?? 'you'})</span>
      }
      return (
        <ActionsMenu
          user={user}
          onDisable={setDisableTarget}
          onReset={setResetTarget}
          onEnable={onEnable}
        />
      )
    }
    return null
  }

  const hasActiveFilters = q || statusFilter !== 'all' || roleFilter !== 'all'

  return (
    <div className="space-y-4">
      {/* 面包屑 */}
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('users')}</h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchUsers' as never) ?? 'Search email...'}
          defaultValue={q}
          onChange={(e) => {
            // 300ms 防抖（§11.B.4）通过 debounce effect 简化：直接 push
            updateQuery({ q: e.target.value, page: 1 })
          }}
        />
        <Select
          value={statusFilter}
          onValueChange={(v) => updateQuery({ status: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="active">{t('active')}</SelectItem>
            <SelectItem value="disabled">{t('disabled')}</SelectItem>
          </SelectContent>
        </Select>
        <Select
          value={roleFilter}
          onValueChange={(v) => updateQuery({ role: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="user">{t('user' as never) ?? 'User'}</SelectItem>
            <SelectItem value="admin">{t('superuser')}</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/users')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={filteredUsers}
        rowKey={(u) => u.id}
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">{t('empty' as never) ?? 'No users'}</p>
        }
        pagination={{
          page,
          pageSize,
          total,
          onPageChange: (p) => updateQuery({ page: p }),
          onPageSizeChange: (s) => updateQuery({ pageSize: s, page: 1 }),
        }}
        mobile={{
          primary: (u) => <span className={u.is_active ? '' : 'opacity-60'}>{u.email}</span>,
          secondary: [
            { key: 'status', label: t('status') },
            { key: 'role', label: t('role') },
            { key: 'last_login', label: t('lastLogin') },
          ],
          actions: (u) =>
            currentUser?.id === u.id ? (
              <span className="text-ink-subtle text-xs">({t('you' as never) ?? 'you'})</span>
            ) : (
              <ActionsMenu
                user={u}
                onDisable={setDisableTarget}
                onReset={setResetTarget}
                onEnable={onEnable}
              />
            ),
        }}
      />

      {/* L3 禁用用户 ConfirmDialog（含封禁原因 input，§5.4） */}
      <ConfirmDialog
        open={!!disableTarget}
        onConfirm={onDisable}
        onClose={() => {
          setDisableTarget(null)
          setDisableReason('')
        }}
        variant="danger"
        title={t('disableDialogTitle')}
        description={t('disableDialogDesc', { email: disableTarget?.email ?? '' })}
        confirmText={t('disable')}
      >
        <div className="mt-4 space-y-1.5">
          <label className="text-ink-muted text-sm font-medium">{t('disableDialogReason')}</label>
          <Input
            value={disableReason}
            onChange={(e) => setDisableReason(e.target.value)}
            placeholder={t('disableDialogReasonPlaceholder')}
          />
        </div>
      </ConfirmDialog>

      {/* L3 重置密码 ConfirmDialog（含新密码 input，§5.4） */}
      <ConfirmDialog
        open={!!resetTarget}
        onConfirm={onResetPassword}
        onClose={() => {
          setResetTarget(null)
          setNewPassword('')
        }}
        variant="danger"
        title={t('resetDialogTitle' as never) ?? t('resetPassword')}
        description={t('resetDialogDesc' as never) ?? `Reset password for "${resetTarget?.email}"?`}
        confirmText={t('resetPassword')}
      >
        <div className="mt-4 space-y-1.5">
          <label className="text-ink-muted text-sm font-medium">
            {t('resetDialogPasswordLabel' as never) ?? 'New password (min 8):'}
          </label>
          <Input
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            minLength={8}
          />
        </div>
      </ConfirmDialog>
    </div>
  )
}

function ActionsMenu({
  user,
  onDisable,
  onReset,
  onEnable,
}: {
  user: AdminUser
  onDisable: (u: AdminUser) => void
  onReset: (u: AdminUser) => void
  onEnable: (u: AdminUser) => void
}) {
  const t = useTranslations('admin')
  return (
    <Menu>
      <Menu.Trigger className="text-ink-muted hover:bg-surface-2 rounded-md p-1">
        <MoreVertical className="h-4 w-4" />
      </Menu.Trigger>
      <Menu.Popup>
        {user.is_active ? (
          <Menu.Item variant="destructive" onClick={() => onDisable(user)}>
            {t('disable')}
          </Menu.Item>
        ) : (
          <Menu.Item onClick={() => onEnable(user)}>{t('enable')}</Menu.Item>
        )}
        <Menu.Separator />
        <Menu.Item onClick={() => onReset(user)}>{t('resetPassword')}</Menu.Item>
      </Menu.Popup>
    </Menu>
  )
}
