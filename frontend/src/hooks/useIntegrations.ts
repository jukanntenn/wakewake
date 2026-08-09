'use client'

// Integration react-query hooks（api-design.md §1.4 集成端点）。

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'

export const integrationsQueryKey = ['integrations'] as const

export function useIntegrations() {
  return useQuery({
    queryKey: integrationsQueryKey,
    queryFn: async () => {
      const resp = await api.integrations.list()
      return resp.items
    },
    // device-sync-v3 §14.3：任一集成处于 connecting（首次对账未完成）时快轮询（3s）；
    // 全一致时慢轮询（15s，刷新 MQTT 连接态变化与 drift 告警展示）。
    refetchInterval: (query) => {
      const items = query.state.data
      if (items?.some((i) => i.status === 'connecting')) {
        return 3000
      }
      return 15000
    },
  })
}

export function useIntegrationSchema(provider: string) {
  return useQuery({
    queryKey: ['integrations', 'schema', provider],
    queryFn: () => api.integrations.schema(provider),
    enabled: !!provider,
  })
}

export function useCreateIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: { provider: string; config: Record<string, unknown>; enabled?: boolean }) =>
      api.integrations.create(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: integrationsQueryKey }),
  })
}

export function usePatchIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({
      provider,
      data,
    }: {
      provider: string
      data: { config?: Record<string, unknown>; enabled?: boolean }
    }) => api.integrations.patch(provider, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: integrationsQueryKey }),
  })
}

export function useDeleteIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (provider: string) => api.integrations.delete(provider),
    onSuccess: () => qc.invalidateQueries({ queryKey: integrationsQueryKey }),
  })
}

export function useToggleIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ provider, enable }: { provider: string; enable: boolean }) =>
      enable ? api.integrations.enable(provider) : api.integrations.disable(provider),
    onSuccess: () => qc.invalidateQueries({ queryKey: integrationsQueryKey }),
  })
}
