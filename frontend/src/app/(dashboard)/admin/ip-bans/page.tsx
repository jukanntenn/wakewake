'use client'

// Admin IP 封禁页（admin-risk-controls WRFC）。
// 精确 IP / CIDR 条目 + TTL（1h/24h/7d/永久）；自封守卫在服务端
// （目标覆盖请求者 IP → 422），前端透出错误码。

import { useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, Loader2, ShieldBan } from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useIpBans, useAddIpBan, useRemoveIpBan } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { EmptyState } from '@/components/ui/empty-state'

type Ttl = '1' | '24' | '168' | 'permanent'

export default function AdminIpBansPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const { data: bans, isLoading } = useIpBans()
  const addMut = useAddIpBan()
  const removeMut = useRemoveIpBan()

  const [target, setTarget] = useState('')
  const [reason, setReason] = useState('')
  const [ttl, setTtl] = useState<Ttl>('24')

  const onAdd = async () => {
    try {
      await addMut.mutateAsync({
        target: target.trim(),
        reason: reason.trim() || undefined,
        ttl_hours: ttl === 'permanent' ? null : Number.parseInt(ttl, 10),
      })
      toast.success(t('ipBans.added'))
      setTarget('')
      setReason('')
    } catch (err) {
      if (err instanceof ApiError) {
        // 自封守卫（target=self_ban 字段码）给专属文案，其余走通用错误表。
        const selfBan = (err.errors ?? []).some((e) => e.code === 'self_ban')
        toast.error(selfBan ? t('ipBans.selfBan') : tErr(err.code as never))
      } else {
        toast.error(tErr('INTERNAL_ERROR'))
      }
    }
  }

  const onRemove = async (id: string) => {
    try {
      await removeMut.mutateAsync(id)
      toast.success(t('ipBans.removed'))
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink flex items-center gap-2 text-2xl font-semibold tracking-tight">
        <ShieldBan className="h-5 w-5" />
        {t('ipBans.title')}
      </h1>
      <p className="text-ink-muted text-sm">{t('ipBans.desc')}</p>

      {/* 添加封禁 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('ipBans.add')}</CardTitle>
          <CardDescription>{t('ipBans.desc')}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="space-y-1.5">
            <label className="text-ink-muted text-sm font-medium">{t('ipBans.target')}</label>
            <input
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              placeholder={t('ipBans.targetPlaceholder')}
              className="border-hairline bg-surface-1 text-ink focus:border-primary placeholder:text-ink-subtle/60 focus:ring-primary/20 h-9 w-72 rounded-md border px-2.5 text-sm outline-none focus:ring-2"
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-ink-muted text-sm font-medium">{t('ipBans.reason')}</label>
            <input
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              className="border-hairline bg-surface-1 text-ink focus:border-primary focus:ring-primary/20 h-9 w-64 rounded-md border px-2.5 text-sm outline-none focus:ring-2"
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-ink-muted text-sm font-medium">{t('ipBans.ttl')}</label>
            <select
              value={ttl}
              onChange={(e) => setTtl(e.target.value as Ttl)}
              className="border-hairline bg-surface-1 text-ink focus:border-primary focus:ring-primary/20 h-9 rounded-md border px-2.5 text-sm outline-none focus:ring-2"
            >
              <option value="1">{t('ipBans.ttl1h')}</option>
              <option value="24">{t('ipBans.ttl24h')}</option>
              <option value="168">{t('ipBans.ttl7d')}</option>
              <option value="permanent">{t('ipBans.ttlPermanent')}</option>
            </select>
          </div>
          <Button onClick={onAdd} disabled={addMut.isPending || target.trim() === ''}>
            {addMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            {t('ipBans.add')}
          </Button>
        </CardContent>
      </Card>

      {/* 封禁列表 */}
      <div className="border-hairline bg-surface-1 overflow-hidden rounded-lg border">
        {isLoading ? (
          <div className="text-ink-muted px-4 py-6 text-center text-sm">
            <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
          </div>
        ) : !bans || bans.length === 0 ? (
          <EmptyState icon={<ShieldBan className="h-8 w-8" />} title={t('ipBans.empty')} />
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-hairline text-ink-muted border-b text-left text-xs tracking-wider uppercase">
                <th className="px-4 py-2 font-medium">{t('ipBans.colTarget')}</th>
                <th className="px-4 py-2 font-medium">{t('ipBans.colReason')}</th>
                <th className="px-4 py-2 font-medium">{t('ipBans.colCreated')}</th>
                <th className="px-4 py-2 font-medium">{t('ipBans.colExpires')}</th>
                <th className="px-4 py-2 font-medium">{t('ipBans.colActions')}</th>
              </tr>
            </thead>
            <tbody>
              {bans.map((b) => (
                <tr key={b.id} className="border-hairline border-b last:border-b-0">
                  <td className="text-ink px-4 py-2.5 font-mono">
                    {b.target}
                    {b.expired && (
                      <span className="text-ink-subtle ml-2 text-xs">
                        ({t('ipBans.expiredTag')})
                      </span>
                    )}
                  </td>
                  <td className="text-ink-muted max-w-48 truncate px-4 py-2.5">
                    {b.reason || '—'}
                  </td>
                  <td className="text-ink-muted px-4 py-2.5">
                    {new Date(b.created_at).toLocaleString()}
                  </td>
                  <td className="text-ink-muted px-4 py-2.5">
                    {b.expires_at ? new Date(b.expires_at).toLocaleString() : t('ipBans.permanent')}
                  </td>
                  <td className="px-4 py-2.5">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => onRemove(b.id)}
                      disabled={removeMut.isPending}
                    >
                      {t('ipBans.remove')}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
