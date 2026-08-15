'use client'

// LocaleProvider：纯客户端自举（frontend/i18n.md §6）。
// 用 NextIntlClientProvider 包裹应用，直接传 locale + messages props（不依赖服务端）。

import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import { NextIntlClientProvider, type AbstractIntlMessages } from 'next-intl'
import enMessages from '@/messages/en.json'
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
  // 首帧用静态 en 兜底（i18n.md §6"首次渲染用 'en' 兜底"的完整实现：
  // 连同 messages 一起兜底,否则预渲染/首帧会渲染出 key 路径而非文案。
  // 代价:en.json 约 8KB gzip 进主包,换来首屏永远是有效文案）。hydration
  // 后切换到用户偏好 locale（懒加载 chunk）。
  const [locale, setLocaleState] = useState<Locale>('en')
  const [messages, setMessages] = useState<AbstractIntlMessages>(enMessages)

  useEffect(() => {
    const detected = getDefaultLocale()
    loadMessages(detected).then((m) => {
      setLocaleState(detected)
      setMessages(m)
      // §7.4：首次自动检测路径同步 <html lang>（修复首屏 lang 错误窗口）。
      document.documentElement.lang = detected
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
