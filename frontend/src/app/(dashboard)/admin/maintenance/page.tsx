'use client'

// Admin Maintenance 页（ui-ux-risk-control §9.10）。
// 显示当前维护状态；未启用时展示「启用维护」表单（severity + message），启用时展示「停用维护」按钮。

import { useState } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { ChevronLeft, Loader2 } from 'lucide-react'
import { ApiError } from '@/lib/api'
import { useMaintenanceStatus, useSetMaintenance } from '@/hooks/useAdmin'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { StatusIndicator } from '@/components/ui/status-indicator'

type Severity = 'registration_disabled' | 'readonly' | 'full'

const SEVERITIES: {
  value: Severity
  labelKey: string
  defaultLabel: string
  descKey: string
  defaultDesc: string
}[] = [
  {
    value: 'registration_disabled',
    labelKey: 'maintenance.severityRegistration',
    defaultLabel: 'Pause registration',
    descKey: 'maintenance.severityRegistrationDesc',
    defaultDesc: 'New sign-ups blocked. Existing users unaffected.',
  },
  {
    value: 'readonly',
    labelKey: 'maintenance.severityReadonly',
    defaultLabel: 'Read-only',
    descKey: 'maintenance.severityReadonlyDesc',
    defaultDesc: 'All write operations blocked. Users can log in and view data.',
  },
  {
    value: 'full',
    labelKey: 'maintenance.severityFull',
    defaultLabel: 'Full lockdown',
    descKey: 'maintenance.severityFullDesc',
    defaultDesc: 'All access blocked except admin. Use for major incidents.',
  },
]

export default function AdminMaintenancePage() {
  const t = useTranslations('admin')
  const tErr = useTranslations('error')
  const { data: status, isLoading } = useMaintenanceStatus()
  const setMut = useSetMaintenance()

  const [severity, setSeverity] = useState<Severity>('registration_disabled')
  const [message, setMessage] = useState('')

  const enabled = status?.enabled ?? false

  const onEnable = async () => {
    try {
      await setMut.mutateAsync({ enabled: true, mode: severity, message: message || undefined })
      toast.success(t('maintenance.enabledToast' as never) ?? 'Maintenance enabled')
      setMessage('')
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const onDisable = async () => {
    try {
      await setMut.mutateAsync({ enabled: false, mode: status?.mode ?? 'registration_disabled' })
      toast.success(t('maintenance.disabledToast' as never) ?? 'Maintenance disabled')
    } catch (err) {
      toast.error(err instanceof ApiError ? tErr(err.code as never) : tErr('INTERNAL_ERROR'))
    }
  }

  const current = SEVERITIES.find((s) => s.value === status?.mode)

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
        {t('maintenance.title' as never) ?? t('maintenance' as never) ?? 'Maintenance'}
      </h1>
      <p className="text-ink-muted text-sm">
        {t('maintenanceDesc' as never) ??
          'Control maintenance mode to restrict access during incidents.'}
      </p>

      {/* 当前状态 */}
      <div className="border-hairline bg-surface-1 flex items-center gap-2.5 rounded-lg border px-4 py-3">
        {isLoading ? (
          <Loader2 className="text-ink-muted h-4 w-4 animate-spin" />
        ) : enabled ? (
          <>
            <StatusIndicator color="warning" />
            <span className="text-ink text-sm font-medium">
              {t('maintenance.active' as never) ?? 'Maintenance active'}
            </span>
            {status && (
              <span className="text-ink-muted text-sm">
                ·{' '}
                {current
                  ? ((t(current.labelKey as never) as string | undefined) ?? current.defaultLabel)
                  : status.mode}
                {status.message ? ` — ${status.message}` : ''}
              </span>
            )}
          </>
        ) : (
          <>
            <StatusIndicator color="success" />
            <span className="text-ink-muted text-sm">
              {t('maintenance.normal' as never) ?? 'Normal — no maintenance active'}
            </span>
          </>
        )}
      </div>

      {/* 启用表单（未启用时）/ 停用按钮（已启用时） */}
      {enabled ? (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">
              {t('maintenance.disableTitle' as never) ?? 'Disable maintenance'}
            </CardTitle>
            <CardDescription>
              {t('maintenance.disableDesc' as never) ??
                'Restore normal access. All users regain their usual permissions.'}
            </CardDescription>
          </CardHeader>
          <CardFooter>
            <Button variant="destructive" onClick={onDisable} disabled={setMut.isPending}>
              {setMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('maintenance.disable' as never) ?? 'Disable maintenance'}
            </Button>
          </CardFooter>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">
              {t('maintenance.enableTitle' as never) ?? 'Enable maintenance'}
            </CardTitle>
            <CardDescription>
              {t('maintenance.enableDesc' as never) ??
                'Choose a severity level. A message is optional but recommended.'}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            {/* Severity 选择 */}
            <div className="space-y-2">
              <label className="text-ink-muted text-sm font-medium">
                {t('maintenance.severity' as never) ?? 'Severity'}
              </label>
              <Select
                value={severity}
                onValueChange={(v) => setSeverity((v ?? severity) as Severity)}
              >
                <SelectTrigger className="w-full sm:w-72">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SEVERITIES.map((s) => (
                    <SelectItem key={s.value} value={s.value}>
                      {t(s.labelKey as never) ?? s.defaultLabel}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-ink-subtle text-xs">
                {t(SEVERITIES.find((s) => s.value === severity)!.descKey as never) ??
                  SEVERITIES.find((s) => s.value === severity)!.defaultDesc}
              </p>
            </div>

            {/* Message（可选） */}
            <div className="space-y-2">
              <label className="text-ink-muted text-sm font-medium">
                {t('maintenance.message' as never) ?? 'Message (optional)'}
              </label>
              <textarea
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                rows={3}
                placeholder={
                  (t('maintenance.messagePlaceholder' as never) as string | undefined) ??
                  'Shown to users on the maintenance screen.'
                }
                className="border-hairline bg-canvas ring-offset-canvas placeholder:text-ink-muted focus-visible:ring-primary flex w-full rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
              />
            </div>
          </CardContent>
          <CardFooter className="gap-2">
            <Button variant="outline" onClick={() => setMessage('')} disabled={setMut.isPending}>
              {t('cancel' as never) ?? 'Cancel'}
            </Button>
            <Button
              onClick={onEnable}
              disabled={setMut.isPending}
              className="bg-warning text-on-warning hover:bg-warning/90"
            >
              {setMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('maintenance.enable' as never) ?? 'Enable maintenance'}
            </Button>
          </CardFooter>
        </Card>
      )}
    </div>
  )
}
