'use client'

// Agent react-query hooks（api-design.md §1.4 Agent 端点）。

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'

export const defaultAgentQueryKey = ['agents', 'default'] as const

export function useDefaultAgent() {
  return useQuery({
    queryKey: defaultAgentQueryKey,
    queryFn: () => api.agents.getDefault(),
    // 5s 自动刷新：agent online/offline 状态变化（SSE 连接建立/断开）需要近实时反映。
    // 状态派生自 hub 内存（is_online），DB 不持久化，必须轮询。
    refetchInterval: 5000,
    refetchOnWindowFocus: true,
    staleTime: 3000,
  })
}

export function useRotatePairingCode() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => api.agents.rotatePairingCode(),
    // rotate 返回完整新码，直接更新缓存（不 invalidate，避免 get_default 的脱敏码覆盖）。
    onSuccess: (data) => {
      qc.setQueryData<import('@/lib/api').DefaultAgent>(defaultAgentQueryKey, (old) =>
        old ? { ...old, pairing_code: data.pairing_code } : old,
      )
    },
  })
}
