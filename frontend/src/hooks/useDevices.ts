'use client'

// Device react-query hooks（api-design.md §1.4 设备端点 + §1.7 唤醒交互流）。
// TanStack Query 管所有 server state，mutation 后 invalidateQueries。

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api, type CreateDeviceInput, type UpdateDeviceInput } from '@/lib/api'

export const devicesQueryKey = ['devices'] as const

export function useDevices() {
  // device-sync-v3 §14.3：有 syncing 态时快轮询（3s），全一致时慢轮询（15s）。
  return useQuery({
    queryKey: devicesQueryKey,
    queryFn: async () => {
      const resp = await api.devices.list()
      return resp.items
    },
    refetchInterval: (query) => {
      const devices = query.state.data
      if (!devices) return false
      // 快轮询触发：投影/云同步中，或设备等待首次云端观测（集成多半 connecting）
      const hasSyncing = devices.some(
        (d) =>
          d.projection_status === 'syncing' ||
          d.cloud_status === 'syncing' ||
          d.cloud_status === 'not_observed',
      )
      return hasSyncing ? 3000 : 15000
    },
  })
}

export function useCreateDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateDeviceInput) => api.devices.create(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: devicesQueryKey }),
  })
}

export function usePatchDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ did, data }: { did: string; data: UpdateDeviceInput }) =>
      api.devices.patch(did, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: devicesQueryKey }),
  })
}

export function useDeleteDevice() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (did: string) => api.devices.delete(did),
    onSuccess: () => qc.invalidateQueries({ queryKey: devicesQueryKey }),
  })
}

/// 唤醒：POST /devices/:did/wake → 202 + command_id → 轮询 GET /commands/:id。
/// 轮询策略（api-design.md §1.7）：首次 500ms，指数退避（500ms→1s→2s），上限 5s，总超时 65s。
export function useWakeDevice() {
  return useMutation({
    mutationFn: async (did: string) => {
      const cmd = await api.devices.wake(did)
      return cmd.command_id
    },
  })
}

/// 轮询命令状态直到终态（completed/failed/expired）或超时 65s。
export async function pollCommandStatus(
  commandId: string,
  onUpdate: (status: string) => void,
): Promise<{ status: string; success?: boolean; message?: string }> {
  const deadline = Date.now() + 65_000
  let delay = 500
  while (Date.now() < deadline) {
    await sleep(delay)
    const cmd = await api.commands.get(commandId)
    onUpdate(cmd.status)
    if (cmd.status === 'completed' || cmd.status === 'failed' || cmd.status === 'expired') {
      return {
        status: cmd.status,
        success: cmd.result?.success,
        message: cmd.result?.message,
      }
    }
    delay = Math.min(delay * 2, 5000)
  }
  return { status: 'expired' }
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
