'use client'

// Admin Activity 页（ui-ux-risk-control §9.5）。
// 统一时间线：login_events + admin_actions（GET /admin/activity）。
// 搜索 + kind（All/Logins/Admin actions）+ result（All/Success/Failed，仅 kind=login 时有效）+ 分页。

import { Suspense } from 'react'
import Link from 'next/link'
import { useSearchParams, useRouter } from 'next/navigation'
import { useTranslations } from 'next-intl'
import { ChevronLeft } from 'lucide-react'
import { useAdminActivity } from '@/hooks/useAdmin'
import { summarizeUA } from '@/lib/ua'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { DataTable, type Column } from '@/components/ui/data-table'
import { StatusIndicator } from '@/components/ui/status-indicator'
import type { ActivityItem } from '@/lib/api'
import { RelativeTime } from '@/components/ui/relative-time'

const DEFAULT_PAGE_SIZE = 20

function ActivityPageInner() {
  const t = useTranslations('admin')
  const router = useRouter()
  const searchParams = useSearchParams()

  // URL query 同步（§11.B.4）：搜索词、kind、result、页码同步到 URL（camelCase）。
  const q = searchParams.get('q') ?? ''
  const kindFilter = searchParams.get('kind') ?? 'all' // all / login / audit
  const resultFilter = searchParams.get('result') ?? 'all' // all / success / failed（仅 kind=login 有效）
  const page = Number(searchParams.get('page') ?? '1')
  const pageSize = Number(searchParams.get('pageSize') ?? String(DEFAULT_PAGE_SIZE))

  // hook/api 使用 snake_case 参数。
  const { data: resp, isLoading } = useAdminActivity({
    q: q || undefined,
    kind: kindFilter === 'all' ? undefined : (kindFilter as 'login' | 'audit'),
    result: resultFilter === 'all' ? undefined : (resultFilter as 'success' | 'failed'),
    page,
    page_size: pageSize,
  })
  const items = resp?.items ?? []
  const total = resp?.total ?? 0

  const updateQuery = (updates: Record<string, string | number>) => {
    const params = new URLSearchParams(searchParams.toString())
    for (const [k, v] of Object.entries(updates)) {
      if (v === 'all' || v === '' || v === 1) {
        params.delete(k)
      } else {
        params.set(k, String(v))
      }
    }
    router.push(`/admin/activity?${params.toString()}`)
  }

  const columns: Column<ActivityItem>[] = [
    { key: 'created_at', label: t('colTime') },
    { key: 'actor', label: t('colActor') },
    { key: 'type', label: t('colAction') },
    { key: 'detail', label: t('colDetail') },
  ]

  const renderCell = (item: ActivityItem, key: string) => {
    if (key === 'created_at') {
      return (
        <span className="text-ink-subtle text-sm whitespace-nowrap">
          <RelativeTime date={item.created_at} />
        </span>
      )
    }
    if (key === 'actor') {
      return (
        <span className="text-ink text-sm">
          {item.actor_label}
          {item.actor_id === null && (
            <span className="text-ink-subtle ml-1"> ({t('unknown' as never) ?? 'unknown'})</span>
          )}
        </span>
      )
    }
    if (key === 'type') {
      if (item.kind === 'audit') {
        // success（enable）→ green；disable → error；其余 neutral
        const action = item.action
        const color = /enable/i.test(action)
          ? 'success'
          : /disable/i.test(action)
            ? 'error'
            : 'neutral'
        return (
          <span className="inline-flex items-center gap-1.5 text-sm">
            <StatusIndicator color={color} />
            <code className="bg-surface-2 text-ink rounded px-1.5 py-0.5 text-xs">{action}</code>
          </span>
        )
      }
      // login：成功 green 实心点；失败 neutral 空心（用 neutral 色 + 自定义空心标记）
      const success = !item.detail.failure_code
      return (
        <span className="inline-flex items-center gap-1.5 text-sm">
          {success ? (
            <>
              <StatusIndicator color="success" />
              <span className="text-ink-muted">
                {t('loginSuccess' as never) ?? 'login success'}
              </span>
            </>
          ) : (
            <>
              <StatusIndicator color="neutral" />
              <span className="text-ink-muted">{t('loginFailed' as never) ?? 'login failed'}</span>
            </>
          )}
        </span>
      )
    }
    if (key === 'detail') {
      if (item.kind === 'login') {
        const parts: string[] = []
        if (item.detail.ip) parts.push(item.detail.ip)
        const ua = summarizeUA(item.detail.user_agent)
        if (ua && ua !== 'Unknown') parts.push(ua)
        return (
          <span className="text-ink-muted text-sm">
            {parts.length > 0 ? parts.join(' · ') : (t('unknown' as never) ?? '—')}
          </span>
        )
      }
      // audit：target + reason
      const parts: string[] = []
      if (item.detail.target) parts.push(item.detail.target)
      if (item.detail.reason) parts.push(item.detail.reason)
      return (
        <span className="text-ink-muted text-sm">{parts.length > 0 ? parts.join(' · ') : '—'}</span>
      )
    }
    return null
  }

  const hasActiveFilters = q || kindFilter !== 'all' || resultFilter !== 'all'

  return (
    <div className="space-y-4">
      {/* 面包屑 */}
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink text-2xl font-semibold tracking-tight">
        {t('activity' as never) ?? 'Activity'}
      </h1>

      {/* 搜索 + 过滤栏 */}
      <div className="flex flex-wrap items-center gap-3">
        <Input
          className="max-w-xs"
          placeholder={t('searchActivity' as never) ?? 'Search actor or detail...'}
          defaultValue={q}
          onChange={(e) => {
            updateQuery({ q: e.target.value, page: 1 })
          }}
        />
        <Select
          value={kindFilter}
          onValueChange={(v) => updateQuery({ kind: v ?? 'all', page: 1 })}
        >
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="login">{t('filterLogins' as never) ?? 'Logins'}</SelectItem>
            <SelectItem value="audit">
              {t('filterAdminActions' as never) ?? 'Admin actions'}
            </SelectItem>
          </SelectContent>
        </Select>
        <Select
          value={resultFilter}
          onValueChange={(v) => updateQuery({ result: v ?? 'all', page: 1 })}
          // result 过滤仅在 kind=login 时有效
          disabled={kindFilter !== 'login'}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('filterAll' as never) ?? 'All'}</SelectItem>
            <SelectItem value="success">{t('filterSuccess' as never) ?? 'Success'}</SelectItem>
            <SelectItem value="failed">{t('filterFailed' as never) ?? 'Failed'}</SelectItem>
          </SelectContent>
        </Select>
        {hasActiveFilters && (
          <button
            onClick={() => router.push('/admin/activity')}
            className="text-ink-muted hover:text-ink text-xs underline"
          >
            {t('filterClear' as never) ?? 'Clear'}
          </button>
        )}
      </div>

      {/* 表格 */}
      <DataTable
        columns={columns}
        rows={items}
        rowKey={(item) =>
          `${item.kind}-${item.created_at}-${item.actor_id ?? 'anon'}-${item.action}`
        }
        render={renderCell}
        loading={isLoading}
        empty={
          <p className="text-ink-muted py-8 text-center">
            {t('emptyActivity' as never) ?? 'No activity yet'}
          </p>
        }
        pagination={{
          page,
          pageSize,
          total,
          onPageChange: (p) => updateQuery({ page: p }),
          onPageSizeChange: (s) => updateQuery({ pageSize: s, page: 1 }),
        }}
        mobile={{
          primary: (item) => (
            <span className="text-ink-subtle">
              <RelativeTime date={item.created_at} />
            </span>
          ),
          secondary: [
            { key: 'actor', label: t('colActor') },
            { key: 'type', label: t('colAction') },
          ],
        }}
      />
    </div>
  )
}

export default function ActivityPage() {
  // Next.js 16：useSearchParams 需要 Suspense 边界（static export）。
  return (
    <Suspense fallback={null}>
      <ActivityPageInner />
    </Suspense>
  )
}
