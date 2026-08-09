import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'

// Mock api 模块（hooks 全部走 api.admin.*）
vi.mock('@/lib/api', () => ({
  api: {
    admin: {
      stats: vi.fn(),
      listAgents: vi.fn(),
      listDevices: vi.fn(),
      listWakes: vi.fn(),
      auditLog: vi.fn(),
      resetUserPassword: vi.fn(),
      verifyUserEmail: vi.fn(),
      resyncDevice: vi.fn(),
      resyncIntegration: vi.fn(),
      disconnectAgent: vi.fn(),
    },
  },
}))

import { api } from '@/lib/api'
import {
  useAdminStats,
  useAdminAgents,
  useAdminDevices,
  useAuditLog,
  useResetUserPassword,
  useVerifyUserEmail,
  useResyncDevice,
  useDisconnectAgent,
} from './useAdmin'

function wrapper(client: QueryClient) {
  const Provider = ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  Provider.displayName = 'TestQueryProvider'
  return Provider
}

function newClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } })
}

describe('useAdmin hooks', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('useAdminStats 调 api.admin.stats 并返回数据', async () => {
    const mockStats = {
      users: 5,
      active_users: 4,
      devices: 3,
      agents: 2,
      integrations: 1,
      wakes: 10,
      devices_syncing: 1,
      devices_sync_error: 0,
      online_agents: 1,
    }
    ;(api.admin.stats as ReturnType<typeof vi.fn>).mockResolvedValue(mockStats)
    const client = newClient()
    const { result } = renderHook(() => useAdminStats(), { wrapper: wrapper(client) })
    await waitFor(() => expect(result.current.data).toEqual(mockStats))
    expect(api.admin.stats).toHaveBeenCalledOnce()
  })

  it('useAdminAgents 调 api.admin.listAgents 带参数', async () => {
    ;(api.admin.listAgents as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: [],
      page: 1,
      page_size: 20,
      total: 0,
    })
    const client = newClient()
    const { result } = renderHook(() => useAdminAgents({ user_id: 7, page: 2 }), {
      wrapper: wrapper(client),
    })
    await waitFor(() => expect(result.current.data?.items).toEqual([]))
    expect(api.admin.listAgents).toHaveBeenCalledWith({ user_id: 7, page: 2 })
  })

  it('useAdminDevices 调 api.admin.listDevices 带 cloud_status 过滤', async () => {
    ;(api.admin.listDevices as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: [],
      page: 1,
      page_size: 20,
      total: 0,
    })
    const client = newClient()
    renderHook(() => useAdminDevices({ cloud_status: 'error' }), { wrapper: wrapper(client) })
    await waitFor(() =>
      expect(api.admin.listDevices).toHaveBeenCalledWith({ cloud_status: 'error' }),
    )
  })

  it('useAuditLog 调 api.admin.auditLog 带 action 过滤', async () => {
    ;(api.admin.auditLog as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: [],
      page: 1,
      page_size: 30,
      total: 0,
    })
    const client = newClient()
    renderHook(() => useAuditLog({ action: 'user.disable' }), { wrapper: wrapper(client) })
    await waitFor(() => expect(api.admin.auditLog).toHaveBeenCalledWith({ action: 'user.disable' }))
  })

  it('useResetUserPassword 调 api.admin.resetUserPassword 传 {id, newPassword}', async () => {
    ;(api.admin.resetUserPassword as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const client = newClient()
    const { result } = renderHook(() => useResetUserPassword(), { wrapper: wrapper(client) })
    await result.current.mutateAsync({ id: 42, newPassword: 'NewStrong123!' })
    expect(api.admin.resetUserPassword).toHaveBeenCalledWith(42, 'NewStrong123!')
  })

  it('useVerifyUserEmail 调 api.admin.verifyUserEmail 传 id', async () => {
    ;(api.admin.verifyUserEmail as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const client = newClient()
    const { result } = renderHook(() => useVerifyUserEmail(), { wrapper: wrapper(client) })
    await result.current.mutateAsync(9)
    expect(api.admin.verifyUserEmail).toHaveBeenCalledWith(9)
  })

  it('useResyncDevice 调 api.admin.resyncDevice 传 did', async () => {
    ;(api.admin.resyncDevice as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const client = newClient()
    const { result } = renderHook(() => useResyncDevice(), { wrapper: wrapper(client) })
    await result.current.mutateAsync('abc-123')
    expect(api.admin.resyncDevice).toHaveBeenCalledWith('abc-123')
  })

  it('useDisconnectAgent 调 api.admin.disconnectAgent 传 id', async () => {
    ;(api.admin.disconnectAgent as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const client = newClient()
    const { result } = renderHook(() => useDisconnectAgent(), { wrapper: wrapper(client) })
    await result.current.mutateAsync(3)
    expect(api.admin.disconnectAgent).toHaveBeenCalledWith(3)
  })

  it('mutation 成功后 invalidate admin queries（stats 重新拉取）', async () => {
    ;(api.admin.stats as ReturnType<typeof vi.fn>).mockResolvedValue({ users: 1 })
    ;(api.admin.disconnectAgent as ReturnType<typeof vi.fn>).mockResolvedValue(undefined)
    const client = newClient()
    const { result: statsResult } = renderHook(() => useAdminStats(), { wrapper: wrapper(client) })
    await waitFor(() => expect(statsResult.current.data).toBeTruthy())
    const firstCalls = (api.admin.stats as ReturnType<typeof vi.fn>).mock.calls.length

    const { result: discResult } = renderHook(() => useDisconnectAgent(), {
      wrapper: wrapper(client),
    })
    await discResult.current.mutateAsync(5)
    // invalidate 触发 stats refetch → stats 调用次数增加
    await waitFor(() => {
      expect((api.admin.stats as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThan(
        firstCalls,
      )
    })
  })
})
