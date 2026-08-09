// i18n 常量（frontend/i18n.md §3）。与后端 8 语言对齐（BCP 47）。

export const availableLocales = ['en', 'zh', 'ja', 'ko', 'de', 'fr', 'es', 'pt'] as const
export type Locale = (typeof availableLocales)[number]
export const defaultLocale: Locale = 'en'

// localStorage key（路径不带 locale，方案 B，i18n.md §2）
const LOCALE_STORAGE_KEY = 'locale'

/**
 * 检测默认 locale（i18n.md §5.1）：
 * localStorage 存储值 → 浏览器语言主码 → 'en'。
 */
export function getDefaultLocale(): Locale {
  if (typeof window === 'undefined') return defaultLocale
  const stored = localStorage.getItem(LOCALE_STORAGE_KEY)
  if (stored && availableLocales.includes(stored as Locale)) return stored as Locale
  const browserLang = navigator.language.split('-')[0]
  if (availableLocales.includes(browserLang as Locale)) return browserLang as Locale
  return defaultLocale
}

/** 持久化 locale（只存 localStorage，不写 cookie，i18n.md §5.2）。 */
export function persistLocale(locale: Locale): void {
  localStorage.setItem(LOCALE_STORAGE_KEY, locale)
}

/** 动态加载 messages（i18n.md §5.3，构建时打成独立 chunk 按需加载）。 */
export async function loadMessages(locale: Locale) {
  return (await import(`@/messages/${locale}.json`)).default
}
