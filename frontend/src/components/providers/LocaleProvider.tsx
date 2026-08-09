'use client'

// LocaleProvider：纯客户端自举（frontend/i18n.md §6）。
// 用 NextIntlClientProvider 包裹应用，直接传 locale + messages props（不依赖服务端）。

import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import { NextIntlClientProvider, type AbstractIntlMessages } from 'next-intl'
import {
  availableLocales,
  getDefaultLocale,
  loadMessages,
  persistLocale,
  type Locale,
} from '@/i18n/constants'

type LocaleContextValue = {
  locale: Locale
  setLocale: (locale: Locale) => Promise<void>
  availableLocales: readonly Locale[]
}

const LocaleContext = createContext<LocaleContextValue | null>(null)

export function useLocaleContext(): LocaleContextValue {
  const ctx = useContext(LocaleContext)
  if (!ctx) {
    throw new Error('useLocaleContext must be used within LocaleProvider')
  }
  return ctx
}

export function LocaleProvider({ children }: { children: React.ReactNode }) {
  // 首次渲染用 'en' 兜底，hydration 后切换到用户偏好 locale（i18n.md §6）。
  const [locale, setLocaleState] = useState<Locale>('en')
  const [messages, setMessages] = useState<AbstractIntlMessages>({})

  useEffect(() => {
    const detected = getDefaultLocale()
    loadMessages(detected).then((m) => {
      setLocaleState(detected)
      setMessages(m)
    })
  }, [])

  const setLocale = useCallback(async (newLocale: Locale) => {
    const m = await loadMessages(newLocale)
    setLocaleState(newLocale)
    setMessages(m)
    persistLocale(newLocale)
    document.documentElement.lang = newLocale
  }, [])

  return (
    <LocaleContext.Provider value={{ locale, setLocale, availableLocales }}>
      <NextIntlClientProvider locale={locale} messages={messages}>
        {children}
      </NextIntlClientProvider>
    </LocaleContext.Provider>
  )
}
