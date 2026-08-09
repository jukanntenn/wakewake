// 单一 API client（api-design.md §1.9）：所有调用走 request<T>()。
// auth token 从 Zustand store 取（不直接读 localStorage）；401 拦截自动 refresh + 单飞去重。
// 错误统一抛 ApiError { code, message, errors }（api-design.md §1.2 / Part 4，i18n.md §8 两层错误码）。

import { useAuthStore } from '@/stores/auth'

const API_BASE = '/api/v1'

// ---- 错误类型（api-design.md Part 4 + i18n.md §8）----

export interface FieldError {
  field: string
  code: string
  params?: Record<string, string>
}

export class ApiError extends Error {
  code: string
  status: number
  errors?: FieldError[]
  constructor(code: string, message: string, status: number, errors?: FieldError[]) {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.status = status
    this.errors = errors
  }
}

// ---- 类型（对齐后端响应信封：单资源直接返回，列表 {items,page,page_size,total}）----

export interface User {
  id: number
  email: string
  is_superuser: boolean
  email_verified?: boolean
}

// 管理后台用户（GET /admin/users 返回，比 User 多 is_active/disabled_at/last_login/created_at）。
export interface AdminUser {
  id: number
  email: string
  is_active: boolean
  disabled_at: string | null
  is_superuser: boolean
  email_verified: boolean
  last_login: string | null
  created_at: string
}

// Admin 全局统计（GET /admin/stats）。
export interface AdminStats {
  users: number
  active_users: number
  devices: number
  agents: number
  integrations: number
  wakes: number
  devices_syncing: number
  devices_sync_error: number
  online_agents: number
}

// Admin 跨用户 agent 视图（GET /admin/agents）。
export interface AdminAgent {
  id: number
  user_id: number
  user_email: string
  aid: string
  name: string
  status: 'pending' | 'online' | 'offline'
  last_seen: string | null
  created_at: string
}

// Admin 跨用户 device 视图（GET /admin/devices）。device-sync-v3：派生 cloud_status。
export interface AdminDevice {
  did: string
  user_id: number
  user_email: string
  name: string
  mac_display: string
  description: string | null
  cloud_status: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
  last_error: string | null
  last_drift_at: string | null
  created_at: string
  updated_at: string
}

// Admin 跨用户 wake 审计（GET /admin/wakes）。
export interface AdminWake {
  id: number
  user_id: number
  user_email: string
  device_did: string
  device_name: string
  type: 'wol' | 'bemfa_wake'
  status: 'success' | 'failed' | 'expired'
  message: string | null
  created_at: string
}

// 审计日志条目（GET /admin/audit-log）。
export interface AuditLogEntry {
  id: number
  actor_id: number
  actor_email: string
  action: string
  target_user_id: number | null
  target_agent_id: number | null
  target_device_did: string | null
  detail: Record<string, unknown>
  created_at: string
}

export interface AuthResponse {
  access_token: string
  refresh_token: string
  expires_in: number
  user: User
}

export interface Device {
  did: string
  name: string
  mac_display: string
  description: string | null
  /** device-sync-v3 §8.3：设备所属 agent 是否在线（hub 连接表）。 */
  agent_online: boolean
  /** §8.3 投影同步状态（syncing | synced | agent_offline）。 */
  projection_status: 'syncing' | 'synced' | 'agent_offline'
  /** §8.3 云端 topic 对账状态（not_observed | syncing | synced | error | no_integration）。 */
  cloud_status: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
  cloud_observed_name: string | null
  cloud_observed_at: string | null
  /** §9.3 漂移告警时间戳（前端 24h 时间过滤，§14.2）。 */
  last_drift_at: string | null
  /** §9.3 漂移类型（deleted | renamed）。 */
  last_drift_kind: 'deleted' | 'renamed' | null
  /** §9.3 单设备修复错误（每轮覆盖）。 */
  last_error: string | null
  created_at: string
  updated_at: string
}

export interface CreateDeviceInput {
  name: string
  mac_encrypted: string
  mac_display: string
  description?: string
  agent_id?: string
}

export type UpdateDeviceInput = Partial<{
  name: string
  mac_encrypted: string
  mac_display: string
  description: string | null
}>

export interface DefaultAgent {
  aid: string
  name: string
  status: 'pending' | 'online' | 'offline'
  pairing_code: string
  public_key: string | null
  last_seen: string | null
  created_at: string
}

export interface Integration {
  provider: string
  config: Record<string, unknown>
  enabled: boolean
  /** device-sync-v3 §8.5 集成 7 态。 */
  status:
    | 'not_connected'
    | 'connecting'
    | 'connected'
    | 'disconnected'
    | 'error'
    | 'disabled'
    | 'agent_offline'
  mqtt_connected: boolean
  last_error?: string
  /** §7.2 最近对账上报时间（仅含 integration 块时刷）。 */
  last_report_at?: string
  created_at: string
  updated_at: string
}

export interface Wake {
  id: number
  device_id: string
  device_name: string
  type: 'wol' | 'bemfa_wake'
  status: 'success' | 'failed' | 'expired'
  message: string | null
  created_at: string
}

export interface Command {
  command_id: string
  type: string
  status: 'pending' | 'dispatched' | 'completed' | 'failed' | 'expired'
  result?: { success: boolean; message: string }
  created_at: string
  completed_at: string | null
}

export interface ListEnvelope<T> {
  items: T[]
  page?: number
  page_size?: number
  total: number
}

// ---- 单飞 refresh（authentication.md §九，并发 401 共享一个 refresh promise）----

let refreshPromise: Promise<void> | null = null

async function singleFlightRefresh(): Promise<void> {
  if (refreshPromise) return refreshPromise
  const refreshToken = useAuthStore.getState().refreshToken
  if (!refreshToken) {
    useAuthStore.getState().logout()
    throw new ApiError('AUTH_REQUIRED', 'No refresh token', 401)
  }
  refreshPromise = (async () => {
    try {
      const resp = await fetch(`${API_BASE}/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
      })
      if (!resp.ok) {
        useAuthStore.getState().logout()
        throw new ApiError('TOKEN_EXPIRED', 'Refresh failed', 401)
      }
      const data: AuthResponse = await resp.json()
      useAuthStore
        .getState()
        .setAuth(data.access_token, useAuthStore.getState().user ?? data.user, data.refresh_token)
    } finally {
      refreshPromise = null
    }
  })()
  return refreshPromise
}

// ---- 核心 request ----

interface RequestOptions {
  method?: string
  headers?: Record<string, string>
  body?: unknown
  // 跳过 401 自动 refresh（refresh 端点自身用）
  skipAuthRefresh?: boolean
}

function getCurrentLocale(): string {
  // 从 DOM 或 localStorage 读 locale（Accept-Language 头，i18n.md §9）
  if (typeof document !== 'undefined') {
    return document.documentElement.lang || 'en'
  }
  return 'en'
}

export async function request<T>(endpoint: string, options: RequestOptions = {}): Promise<T> {
  const { method = 'GET', headers = {}, body, skipAuthRefresh } = options

  const buildConfig = (): RequestInit => {
    const config: RequestInit = {
      method,
      headers: {
        'Content-Type': 'application/json',
        'Accept-Language': getCurrentLocale(),
        ...headers,
      },
    }
    const token = useAuthStore.getState().token
    if (token) {
      ;(config.headers as Record<string, string>).Authorization = `Bearer ${token}`
    }
    if (body) {
      config.body = JSON.stringify(body)
    }
    return config
  }

  let response = await fetch(`${API_BASE}${endpoint}`, buildConfig())

  // 401 拦截自动 refresh + 重试（单飞去重，authentication.md §九）
  if (response.status === 401 && !skipAuthRefresh) {
    try {
      await singleFlightRefresh()
      response = await fetch(`${API_BASE}${endpoint}`, buildConfig())
    } catch {
      // refresh 失败已在 singleFlightRefresh 里 logout
      throw new ApiError('TOKEN_EXPIRED', 'Session expired', 401)
    }
  }

  if (!response.ok) {
    const errBody = await response.json().catch(() => null)
    const code = errBody?.code ?? 'INTERNAL_ERROR'
    const message = errBody?.message ?? 'Request failed'
    throw new ApiError(code, message, response.status, errBody?.errors)
  }

  if (response.status === 204) return undefined as T
  return response.json()
}

// ---- API 函数（对齐 api-design.md §1.4 端点全表）----

// 把可选 query 参数对象序列化成 `?k=v&k=v`（跳过 undefined/null）。空对象返回空串。
function buildQuery(params?: Record<string, unknown>): string {
  if (!params) return ''
  const search = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null) search.set(k, String(v))
  }
  const qs = search.toString()
  return qs ? `?${qs}` : ''
}

export const api = {
  auth: {
    register: (data: { email: string; password: string }) =>
      request<User>('/auth/register', { method: 'POST', body: data }),
    login: (data: { email: string; password: string }) =>
      request<AuthResponse>('/auth/login', { method: 'POST', body: data }),
    refresh: (refreshToken: string) =>
      request<Pick<AuthResponse, 'access_token' | 'refresh_token' | 'expires_in'>>(
        '/auth/refresh',
        {
          method: 'POST',
          body: { refresh_token: refreshToken },
          skipAuthRefresh: true,
        },
      ),
    verifyEmail: (token: string) =>
      request<AuthResponse>('/auth/verify-email', { method: 'POST', body: { token } }),
    resendVerification: (email: string) =>
      request<{ sent: boolean }>('/auth/verify-email/resend', {
        method: 'POST',
        body: { email },
      }),
  },
  user: {
    me: () => request<User>('/me'),
    changePassword: (data: { current_password: string; new_password: string }) =>
      request<void>('/me/password', { method: 'POST', body: data }),
    logout: (refreshToken: string) =>
      request<void>('/me/logout', { method: 'POST', body: { refresh_token: refreshToken } }),
  },
  devices: {
    list: () => request<ListEnvelope<Device>>('/devices'),
    create: (data: CreateDeviceInput) =>
      request<Device>('/devices', { method: 'POST', body: data }),
    get: (did: string) => request<Device>(`/devices/${did}`),
    patch: (did: string, data: UpdateDeviceInput) =>
      request<Device>(`/devices/${did}`, { method: 'PATCH', body: data }),
    delete: (did: string) => request<void>(`/devices/${did}`, { method: 'DELETE' }),
    wake: (did: string) => request<Command>(`/devices/${did}/wake`, { method: 'POST' }),
  },
  commands: {
    get: (id: string) => request<Command>(`/commands/${id}`),
  },
  agents: {
    getDefault: () => request<DefaultAgent>('/agents/default'),
    rotatePairingCode: () =>
      request<{ pairing_code: string }>('/agents/default/pairing-code/rotate', { method: 'POST' }),
  },
  integrations: {
    list: () => request<ListEnvelope<Integration>>('/integrations'),
    schema: (provider: string) =>
      request<Record<string, unknown>>(`/integrations/${provider}/schema`),
    create: (data: { provider: string; config: Record<string, unknown>; enabled?: boolean }) =>
      request<Integration>('/integrations', { method: 'POST', body: data }),
    get: (provider: string) => request<Integration>(`/integrations/${provider}`),
    patch: (provider: string, data: { config?: Record<string, unknown>; enabled?: boolean }) =>
      request<Integration>(`/integrations/${provider}`, { method: 'PATCH', body: data }),
    delete: (provider: string) => request<void>(`/integrations/${provider}`, { method: 'DELETE' }),
    disable: (provider: string) =>
      request<void>(`/integrations/${provider}/disable`, { method: 'POST' }),
    enable: (provider: string) =>
      request<void>(`/integrations/${provider}/enable`, { method: 'POST' }),
  },
  wakes: {
    list: (params?: { before?: string; page_size?: number }) =>
      request<ListEnvelope<Wake>>(
        `/wakes${params ? '?' + new URLSearchParams(params as Record<string, string>).toString() : ''}`,
      ),
  },
  admin: {
    stats: () => request<AdminStats>('/admin/stats'),
    listUsers: (params?: { is_active?: boolean; page?: number; page_size?: number }) =>
      request<ListEnvelope<AdminUser>>(`/admin/users${buildQuery(params)}`),
    disableUser: (id: number) =>
      request<AdminUser>(`/admin/users/${id}/disable`, { method: 'POST' }),
    enableUser: (id: number) => request<AdminUser>(`/admin/users/${id}/enable`, { method: 'POST' }),
    resetUserPassword: (id: number, newPassword: string) =>
      request<void>(`/admin/users/${id}/reset-password`, {
        method: 'POST',
        body: { new_password: newPassword },
      }),
    verifyUserEmail: (id: number) =>
      request<void>(`/admin/users/${id}/verify-email`, { method: 'POST' }),
    listAgents: (params?: { user_id?: number; page?: number; page_size?: number }) =>
      request<ListEnvelope<AdminAgent>>(`/admin/agents${buildQuery(params)}`),
    listDevices: (params?: {
      user_id?: number
      cloud_status?: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
      page?: number
      page_size?: number
    }) => request<ListEnvelope<AdminDevice>>(`/admin/devices${buildQuery(params)}`),
    listWakes: (params?: { user_id?: number; before?: string; page_size?: number }) => {
      // wakes 用游标分页，响应是 { items, page_size, total }（无 page）。
      const qs = buildQuery(params)
      return request<Pick<ListEnvelope<AdminWake>, 'items' | 'page_size' | 'total'>>(
        `/admin/wakes${qs}`,
      )
    },
    resyncDevice: (did: string) =>
      request<void>(`/admin/devices/${did}/resync`, { method: 'POST' }),
    resyncIntegration: (id: number) =>
      request<void>(`/admin/integrations/${id}/resync`, { method: 'POST' }),
    disconnectAgent: (id: number) =>
      request<void>(`/admin/agents/${id}/disconnect`, { method: 'POST' }),
    auditLog: (params?: { action?: string; page?: number; page_size?: number }) =>
      request<ListEnvelope<AuditLogEntry>>(`/admin/audit-log${buildQuery(params)}`),
  },
}
