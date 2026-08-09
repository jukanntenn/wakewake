'use client'

// Admin react-query hooks（风控第一版）。
// stats / 跨用户 agents / devices / wakes / 审计日志 + 风控 mutation。
// mutation 成功后按需 invalidate 相关 query。

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'

export const adminKeys = {
  stats: ['admin', 'stats'] as const,
  agents: ['admin', 'agents'] as const,
  devices: ['admin', 'devices'] as const,
  wakes: ['admin', 'wakes'] as const,
  auditLog: ['admin', 'audit-log'] as const,
}

// ---- 统计 ----

export function useAdminStats() {
  return useQuery({ queryKey: adminKeys.stats, queryFn: () => api.admin.stats() })
}

// ---- 跨用户 agent 列表 ----

export function useAdminAgents(params?: { user_id?: number; page?: number; page_size?: number }) {
  return useQuery({
    queryKey: [...adminKeys.agents, params ?? {}],
    queryFn: () => api.admin.listAgents(params),
  })
}

// ---- 跨用户 device 列表 ----

export function useAdminDevices(params?: {
  user_id?: number
  cloud_status?: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
  page?: number
  page_size?: number
}) {
  return useQuery({
    queryKey: [...adminKeys.devices, params ?? {}],
    queryFn: () => api.admin.listDevices(params),
  })
}

// ---- 跨用户 wake 审计 ----

export function useAdminWakes(params?: { user_id?: number; before?: string; page_size?: number }) {
  return useQuery({
    queryKey: [...adminKeys.wakes, params ?? {}],
    queryFn: () => api.admin.listWakes(params),
  })
}

// ---- 审计日志 ----

export function useAuditLog(params?: { action?: string; page?: number; page_size?: number }) {
  return useQuery({
    queryKey: [...adminKeys.auditLog, params ?? {}],
    queryFn: () => api.admin.auditLog(params),
  })
}

// ---- 风控 mutations ----

function useInvalidateAdmin() {
  const qc = useQueryClient()
  return () => {
    qc.invalidateQueries({ queryKey: ['admin'] })
  }
}

export function useResetUserPassword() {
  const invalidate = useInvalidateAdmin()
  return useMutation({
    mutationFn: ({ id, newPassword }: { id: number; newPassword: string }) =>
      api.admin.resetUserPassword(id, newPassword),
    onSuccess: invalidate,
  })
}

export function useVerifyUserEmail() {
  const invalidate = useInvalidateAdmin()
  return useMutation({
    mutationFn: (id: number) => api.admin.verifyUserEmail(id),
    onSuccess: invalidate,
  })
}

export function useResyncDevice() {
  const invalidate = useInvalidateAdmin()
  return useMutation({
    mutationFn: (did: string) => api.admin.resyncDevice(did),
    onSuccess: invalidate,
  })
}

export function useResyncIntegration() {
  const invalidate = useInvalidateAdmin()
  return useMutation({
    mutationFn: (id: number) => api.admin.resyncIntegration(id),
    onSuccess: invalidate,
  })
}

export function useDisconnectAgent() {
  const invalidate = useInvalidateAdmin()
  return useMutation({
    mutationFn: (id: number) => api.admin.disconnectAgent(id),
    onSuccess: invalidate,
  })
}
