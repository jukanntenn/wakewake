'use client'

// ErrorState：错误状态（§4.7/§12.13）。持久留存（不用 toast）——GET 错误需用户看到并主动重试。

import { TriangleAlert, RefreshCw } from 'lucide-react'
import { useTranslations } from 'next-intl'
import { Button } from '@/components/ui/button'

export interface ErrorStateProps {
  message?: string
  onRetry?: () => void
}

export function ErrorState({ message, onRetry }: ErrorStateProps) {
  const tError = useTranslations('error')
  const tCommon = useTranslations('common')
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <TriangleAlert className="text-destructive mb-4 h-12 w-12" />
      <h3 className="text-ink text-lg font-medium">{message ?? tError('default')}</h3>
      <p className="text-ink-muted mt-1 text-sm">{tError('checkConnection')}</p>
      {onRetry && (
        <Button variant="outline" onClick={onRetry} className="mt-4 gap-1.5">
          <RefreshCw className="h-4 w-4" />
          {tCommon('retry')}
        </Button>
      )}
    </div>
  )
}
