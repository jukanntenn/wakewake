'use client'

// LoadingState：骨架屏，形状匹配内容（§4.7/§12.14）。
// variant: cards（设备网格）/ rows（表格）/ detail（键值行）/ page（居中 spinner）。

import { Loader2 } from 'lucide-react'
import { cn } from '@/lib/utils'

export interface LoadingStateProps {
  variant: 'cards' | 'rows' | 'detail' | 'page'
  count?: number
}

export function LoadingState({ variant, count }: LoadingStateProps) {
  if (variant === 'page') {
    return (
      <div className="flex items-center justify-center py-24">
        <Loader2 className="text-ink-muted h-8 w-8 animate-spin" />
      </div>
    )
  }

  if (variant === 'cards') {
    const n = count ?? 2
    return (
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {Array.from({ length: n }).map((_, i) => (
          <div key={i} className="border-hairline bg-surface-1 rounded-lg border p-6">
            <div className="flex items-start gap-3">
              <div className="bg-surface-2 h-10 w-10 animate-pulse rounded-md" />
              <div className="flex-1 space-y-2">
                <div className="bg-surface-2 h-4 w-32 animate-pulse rounded" />
                <div className="bg-surface-2 h-3 w-40 animate-pulse rounded" />
              </div>
            </div>
            <div className="bg-surface-2 mt-4 h-9 w-full animate-pulse rounded-md" />
          </div>
        ))}
      </div>
    )
  }

  if (variant === 'rows') {
    const n = count ?? 5
    return (
      <div className="space-y-2">
        {Array.from({ length: n }).map((_, i) => (
          <div
            key={i}
            className="border-hairline bg-surface-1 flex items-center gap-4 rounded-md border p-4"
          >
            <div className="bg-surface-2 h-4 flex-1 animate-pulse rounded" />
            <div className="bg-surface-2 h-4 w-20 animate-pulse rounded" />
            <div className="bg-surface-2 h-4 w-16 animate-pulse rounded" />
          </div>
        ))}
      </div>
    )
  }

  // detail：键值行骨架
  return (
    <div className={cn('space-y-3')}>
      {Array.from({ length: count ?? 4 }).map((_, i) => (
        <div key={i} className="border-hairline bg-surface-1 rounded-md border p-4">
          <div className="bg-surface-2 mb-2 h-3 w-24 animate-pulse rounded" />
          <div className="bg-surface-2 h-4 w-48 animate-pulse rounded" />
        </div>
      ))}
    </div>
  )
}
