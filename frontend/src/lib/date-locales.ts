// date-locales：date-fns locale 按需映射（§11.G.1）。
// 每个 locale 单独 import，按需 chunk。配合 next-intl currentLocale 使用。

import { zhCN, enUS, ja, ko, de, fr, es, ptBR, type Locale } from 'date-fns/locale'

import type { Locale as AppLocale } from '@/i18n/constants'

export const dateFnsLocaleMap: Record<AppLocale, Locale> = {
  en: enUS,
  zh: zhCN,
  ja: ja,
  ko: ko,
  de: de,
  fr: fr,
  es: es,
  pt: ptBR,
}
