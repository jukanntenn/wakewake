'use client'

// Provider stack（routing-and-guards.md §"Provider Stack"）：
// LocaleProvider（外层，让所有页面能用 i18n）→ QueryProvider → ThemeProvider → ToastProvider。

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Toaster } from 'sonner'
import { useState } from 'react'
import { ThemeProvider } from '@/providers/theme-provider'
import { LocaleProvider } from '@/components/providers/LocaleProvider'

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 60 * 1000,
          },
        },
      }),
  )

  return (
    <LocaleProvider>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider attribute="class" defaultTheme="system" disableTransitionOnChange>
          {children}
          <Toaster position="top-right" richColors />
        </ThemeProvider>
      </QueryClientProvider>
    </LocaleProvider>
  )
}
