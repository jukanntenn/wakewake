'use client'

// 相对时间（UX-59）：date-fns formatDistanceToNow + locale。
// 全站时间列统一用它替代 toLocaleString（绝对时间）。title 保留绝对时间供 hover。

import { formatDistanceToNow, isValid } from 'date-fns'
import { useLocale } from 'next-intl'

import { dateFnsLocaleMap } from '@/lib/date-locales'
import type { Locale as AppLocale } from '@/i18n/constants'

export function RelativeTime({
  date,
  fallback = '—',
}: {
  date: string | number | Date | null | undefined
  fallback?: string
}) {
  const locale = useLocale() as AppLocale
  if (date === null || date === undefined || date === '') return <>{fallback}</>
  const d = new Date(date)
  if (!isValid(d)) return <>{fallback}</>
  const dfnsLocale = dateFnsLocaleMap[locale] ?? dateFnsLocaleMap.en
  const relative = formatDistanceToNow(d, { addSuffix: true, locale: dfnsLocale })
  return <span title={d.toLocaleString()}>{relative}</span>
}
