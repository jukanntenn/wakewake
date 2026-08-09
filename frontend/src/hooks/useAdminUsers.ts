'use client'

// Admin users react-query hooks（api-design.md §Admin）。
// 列用户 + disable/enable，mutation 后 invalidateQueries。

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api'

export const adminUsersQueryKey = ['admin', 'users'] as const

export function useAdminUsers(filter?: { is_active?: boolean }) {
  return useQuery({
    queryKey: [...adminUsersQueryKey, filter ?? {}],
    queryFn: async () => {
      const resp = await api.admin.listUsers(filter)
      return resp.items
    },
  })
}

export function useDisableUser() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => api.admin.disableUser(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: adminUsersQueryKey }),
  })
}

export function useEnableUser() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => api.admin.enableUser(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: adminUsersQueryKey }),
  })
}
