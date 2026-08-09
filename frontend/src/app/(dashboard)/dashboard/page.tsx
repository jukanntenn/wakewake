'use client'

import { useTranslations } from 'next-intl'
import Link from 'next/link'

export default function DashboardPage() {
  const t = useTranslations('dashboard')
  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>
      <p className="text-ink-muted">{t('overview')}</p>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Link
          href="/devices"
          className="border-hairline bg-surface-1 shadow-card hover:bg-surface-2 rounded-lg border p-6 transition"
        >
          <h2 className="text-ink font-medium">{t('manageDevices')}</h2>
          <p className="text-ink-muted mt-1 text-sm">{t('manageDevicesDesc')}</p>
        </Link>
        <Link
          href="/agents"
          className="border-hairline bg-surface-1 shadow-card hover:bg-surface-2 rounded-lg border p-6 transition"
        >
          <h2 className="text-ink font-medium">{t('setupAgent')}</h2>
          <p className="text-ink-muted mt-1 text-sm">{t('setupAgentDesc')}</p>
        </Link>
        <Link
          href="/integrations"
          className="border-hairline bg-surface-1 shadow-card hover:bg-surface-2 rounded-lg border p-6 transition"
        >
          <h2 className="text-ink font-medium">{t('integrations')}</h2>
          <p className="text-ink-muted mt-1 text-sm">{t('integrationsDesc')}</p>
        </Link>
      </div>
    </div>
  )
}
