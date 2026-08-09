'use client'

// Wake react-query hooks（api-design.md §1.4 唤醒历史，游标分页 ?before=<created_at>）。

import { useQuery } from '@tanstack/react-query'
import { api, type Wake } from '@/lib/api'

/// 首页列表（无 before 游标）：10s 自动刷新，唤醒结果（agent complete 写入）近实时反映。
export function useWakes(before?: string) {
  return useQuery({
    queryKey: ['wakes', before ?? ''],
    queryFn: async () => {
      const resp = await api.wakes.list({ before, page_size: 20 })
      return resp
    },
    // 仅首页自动刷新（加载更多页保持静态，避免游标漂移）。
    refetchInterval: before ? false : 10_000,
    refetchOnWindowFocus: true,
  })
}

export type { Wake }
