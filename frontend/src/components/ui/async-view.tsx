'use client'

// AsyncView：声明式列表状态机（§6/§13.4/§11.A.4/§12.11）。
// 5 态：loading 骨架 / error 持久 / error+stale 保留旧数据+红条 / empty / data + refreshing 绿条。
// 核心修复（§6.3）：GET 失败有旧数据时不擦除，保留旧数据 + 红条 + 重试按钮。

import * as React from 'react'
import { ErrorState } from '@/components/ui/error-state'
import { LoadingState } from '@/components/ui/loading-state'

type AsyncQuery<T> = {
  data: T | undefined
  isLoading: boolean
  isFetching: boolean
  isError: boolean
  error: unknown
  refetch: () => Promise<unknown>
}

export interface AsyncViewProps<T extends unknown[]> {
  query: AsyncQuery<T>
  empty?: React.ReactNode
  loadingVariant?: 'cards' | 'rows' | 'detail' | 'page'
  children: (data: T) => React.ReactNode
}

export function AsyncView<T extends unknown[]>({
  query,
  empty,
  loadingVariant = 'page',
  children,
}: AsyncViewProps<T>) {
  const { data, isLoading, isFetching, isError, refetch } = query
  const hasData = !!data && data.length > 0

  // 1. 首次加载：骨架
  if (isLoading && !hasData) {
    return <LoadingState variant={loadingVariant} />
  }

  // 2. 首次加载失败（无旧数据）：持久 ErrorState
  if (isError && !hasData) {
    return <ErrorState onRetry={() => refetch()} />
  }

  // 3. 有数据但为空：empty prop
  if (!hasData) {
    return <>{empty ?? null}</>
  }

  // 4. 有数据：渲染 children + 顶部细条
  //    - refreshing（后台刷新中）：顶部 2px success 色流动条（§6.4/§12.11）
  //    - error+stale（有数据但刷新失败）：顶部 2px destructive 色条（§11.A.4）
  const showRefreshing = isFetching && !isError
  const showStaleError = isError

  return (
    <div className="relative">
      {showRefreshing && (
        <div className="bg-success absolute -top-14 right-0 left-0 z-0 h-0.5 w-full animate-pulse" />
      )}
      {showStaleError && (
        <div className="bg-destructive absolute -top-14 right-0 left-0 z-0 h-0.5 w-full" />
      )}
      {/* stale error 时附加重试按钮（小图标，不遮挡内容） */}
      {showStaleError && (
        <div className="mb-2 flex justify-end">
          <button onClick={() => refetch()} className="text-destructive text-xs underline">
            ↻
          </button>
        </div>
      )}
      {children(data as T)}
    </div>
  )
}
