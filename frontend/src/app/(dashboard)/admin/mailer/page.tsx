'use client'

// Admin 邮件与注册风控页（admin-risk-controls WRFC）。
// 运行时总闸（POST /admin/mailer {enabled}）、分路日预算（{limits}）、
// PoW 难度旋钮（POST /admin/pow）——即时生效，持久化跨重启。

import { useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, Loader2, Mail, Gauge } from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useMailerStatus, useSetMailer, usePowStatus, useSetPow } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { StatusIndicator } from '@/components/ui/status-indicator'

type Path = 'register' | 'resend' | 'reset'

export default function AdminMailerPage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const { data: status, isLoading } = useMailerStatus()
  const setMut = useSetMailer()
  const { data: pow } = usePowStatus()
  const powMut = useSetPow()

  const [limits, setLimits] = useState<{ register: string; resend: string; reset: string }>({
    register: '',
    resend: '',
    reset: '',
  })
  const [difficulty, setDifficulty] = useState('')

  const onToggle = async (enabled: boolean) => {
    try {
      await setMut.mutateAsync({ enabled })
      toast.success(t('mailer.mailerUpdated'))
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const onSaveLimits = async () => {
    const current = status?.limits
    const num = (v: string, fallback?: number) => {
      const n = Number.parseInt(v, 10)
      return Number.isFinite(n) && n >= 0 ? n : (fallback ?? 0)
    }
    try {
      await setMut.mutateAsync({
        limits: {
          register: num(limits.register, current?.register),
          resend: num(limits.resend, current?.resend),
          reset: num(limits.reset, current?.reset),
        },
      })
      toast.success(t('mailer.saved'))
      setLimits({ register: '', resend: '', reset: '' })
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const onSaveDifficulty = async () => {
    const n = Number.parseInt(difficulty, 10)
    if (!Number.isFinite(n) || n < 0 || n > (pow?.max_difficulty ?? 10)) {
      return
    }
    try {
      await powMut.mutateAsync(n)
      toast.success(t('mailer.powSaved'))
      setDifficulty('')
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const pathRows: { key: Path; labelKey: 'limitRegister' | 'limitResend' | 'limitReset' }[] = [
    { key: 'register', labelKey: 'limitRegister' },
    { key: 'resend', labelKey: 'limitResend' },
    { key: 'reset', labelKey: 'limitReset' },
  ]

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2 text-sm">
        <Link href="/admin" className="text-ink-muted hover:text-ink flex items-center gap-1">
          <ChevronLeft className="h-4 w-4" />
          {t('title')}
        </Link>
      </div>
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('mailer.title')}</h1>
      <p className="text-ink-muted text-sm">{t('mailer.desc')}</p>

      {/* 发信总闸 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-lg">
            <Mail className="h-4 w-4" />
            {t('mailer.switchTitle')}
          </CardTitle>
          <CardDescription>{t('mailer.switchDesc')}</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading || !status ? (
            <Loader2 className="text-ink-muted h-4 w-4 animate-spin" />
          ) : (
            <div className="border-hairline bg-surface-1 flex items-center gap-2.5 rounded-lg border px-4 py-3">
              <StatusIndicator color={status.enabled ? 'success' : 'warning'} />
              <span className="text-ink text-sm font-medium">
                {status.enabled ? t('mailer.on') : t('mailer.off')}
              </span>
              <span className="text-ink-muted text-sm">
                · {t('mailer.sentToday')}:{' '}
                {status.sent.register + status.sent.resend + status.sent.reset}
                {' · '}
                {t('mailer.blockedToday')}:{' '}
                {status.blocked.register + status.blocked.resend + status.blocked.reset}
              </span>
            </div>
          )}
        </CardContent>
        <CardFooter>
          <Button
            variant={status?.enabled ? 'destructive' : 'default'}
            onClick={() => onToggle(!status?.enabled)}
            disabled={setMut.isPending || isLoading || !status}
          >
            {setMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            {status?.enabled ? t('mailer.turnOff') : t('mailer.turnOn')}
          </Button>
        </CardFooter>
      </Card>

      {/* 分路日预算 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('mailer.limitsTitle')}</CardTitle>
          <CardDescription>{t('mailer.limitsDesc')}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {pathRows.map(({ key, labelKey }) => (
            <div key={key} className="flex flex-wrap items-center gap-3">
              <label className="text-ink-muted w-44 text-sm font-medium">
                {t(`mailer.${labelKey}` as never)}
              </label>
              <input
                type="number"
                min={0}
                placeholder={String(status?.limits?.[key] ?? 0)}
                value={limits[key]}
                onChange={(e) => setLimits((s) => ({ ...s, [key]: e.target.value }))}
                className="border-hairline bg-surface-1 text-ink focus:border-primary focus:ring-primary/20 h-9 w-28 rounded-md border px-2.5 text-sm outline-none focus:ring-2"
              />
              <span className="text-ink-subtle text-xs">
                {t('mailer.sentToday')}: {status?.sent?.[key] ?? 0} · {t('mailer.blockedToday')}:{' '}
                {status?.blocked?.[key] ?? 0}
              </span>
            </div>
          ))}
        </CardContent>
        <CardFooter>
          <Button onClick={onSaveLimits} disabled={setMut.isPending}>
            {setMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            {t('mailer.save')}
          </Button>
        </CardFooter>
      </Card>

      {/* PoW 难度旋钮 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-lg">
            <Gauge className="h-4 w-4" />
            {t('mailer.powTitle')}
          </CardTitle>
          <CardDescription>{t('mailer.powDesc')}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <span className="text-ink-muted text-sm">
              {t('riskPow')}: <span className="text-ink font-medium">{pow?.difficulty ?? '—'}</span>
            </span>
            <input
              type="number"
              min={0}
              max={pow?.max_difficulty ?? 10}
              placeholder={String(pow?.difficulty ?? 4)}
              value={difficulty}
              onChange={(e) => setDifficulty(e.target.value)}
              className="border-hairline bg-surface-1 text-ink focus:border-primary focus:ring-primary/20 h-9 w-24 rounded-md border px-2.5 text-sm outline-none focus:ring-2"
            />
          </div>
        </CardContent>
        <CardFooter>
          <Button onClick={onSaveDifficulty} disabled={powMut.isPending || difficulty === ''}>
            {powMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            {t('mailer.powSave')}
          </Button>
        </CardFooter>
      </Card>
    </div>
  )
}
