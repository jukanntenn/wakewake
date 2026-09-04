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

export interface UserLimits {
  max_devices: number
}

export interface User {
  id: number
  email: string
  is_superuser: boolean
  email_verified?: boolean
  // domain 常量投影（agent-onboarding.md）：旧持久化会话缺失时跳过前置配额判断，服务端兜底。
  limits?: UserLimits
}

// 管理后台用户（GET /admin/users 返回，比 User 多 is_active/disabled_at/last_login/created_at）。
export interface AdminUser {
  id: number
  email: string
  is_active: boolean
  disabled_at: string | null
  disabled_reason: string | null
  disabled_by: number | null
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

// Admin 跨用户 integration（ui-ux-risk-control §9.8）。
export interface AdminIntegration {
  id: number
  provider: string
  user_id: number
  user_email: string
  status: Integration['status']
  mqtt_connected: boolean
  last_error: string | null
  last_report_at: string | null
  created_at: string
}

// Activity 统一时间线项（ui-ux-risk-control §0.3）。
export interface ActivityItem {
  kind: 'login' | 'audit'
  created_at: string
  actor_id: number | null
  actor_label: string
  action: string
  detail: {
    ip: string | null
    user_agent: string | null
    failure_code: string | null
    target: string | null
    reason: string | null
  }
}

// 维护模式状态（ui-ux-risk-control §8.4/§9.10）。
export interface MaintenanceStatus {
  enabled: boolean
  mode: 'registration_disabled' | 'readonly' | 'full'
  message: string
}

// 邮件发信运行态（GET/POST /admin/mailer，admin-risk-controls WRFC）。
// sent/blocked 为当日 UTC 分路计数；limits 0 = 不限。
export interface MailerControlStatus {
  enabled: boolean
  limits: { register: number; resend: number; reset: number }
  day: string
  sent: { register: number; resend: number; reset: number }
  blocked: { register: number; resend: number; reset: number }
}

// PoW 难度旋钮（GET/POST /admin/pow）。
export interface PowStatus {
  difficulty: number
  max_difficulty: number
}

// 应用层 IP 封禁条目（GET/POST /admin/ip-bans）。
export interface IpBanEntry {
  id: string
  target: string
  kind: 'ip' | 'cidr'
  reason: string
  created_by: number
  created_at: string
  expires_at: string | null
  expired: boolean
}

// 风控聚合面板（GET /admin/risk）。
export interface RiskOverview {
  registrations_24h: number
  registrations_7d: number
  unverified_count: number
  oldest_unverified_age_hours: number | null
  failed_logins_24h: number
  top_failed_ips: { ip: string; failures: number; distinct_emails: number }[]
  top_failed_emails: { email: string; failures: number; distinct_ips: number }[]
  mailer: MailerControlStatus
  pow_difficulty: number
  ip_ban_count: number
  rate_limited_since_start: number
  rate_limited_uptime_secs: number
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
  /** §3.7 详情面板 CONNECTION 区：所属 agent 名称（agent 行缺失时为 null）。 */
  agent_name: string | null
  /** §3.7 详情面板 CONNECTION 区：所属 agent 最近在线时间（Rfc3339）。 */
  agent_last_seen: string | null
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
  // 部分 mutating 端点（集成 enable/disable、admin resync/disconnect 等）以 200 + 空 body
  // 回应。空 body 不能当 JSON 解析（会抛 SyntaxError 而误报失败），统一按 void 处理。
  const text = await response.text()
  if (text.length === 0) return undefined as T
  return JSON.parse(text) as T
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
    register: (data: { email: string; password: string; challenge: string; nonce: string }) =>
      request<User>('/auth/register', { method: 'POST', body: data, skipAuthRefresh: true }),
    login: (data: { email: string; password: string }) =>
      request<AuthResponse>('/auth/login', { method: 'POST', body: data, skipAuthRefresh: true }),
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
    powChallenge: () =>
      request<{ id: string; challenge: string; difficulty: number }>('/pow/challenge'),
    requestPasswordReset: (data: { email: string; challenge: string; nonce: string }) =>
      request<void>('/auth/password-reset/request', { method: 'POST', body: data }),
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
    listUsers: (params?: { is_active?: boolean; q?: string; page?: number; page_size?: number }) =>
      request<ListEnvelope<AdminUser>>(`/admin/users${buildQuery(params)}`),
    disableUser: (id: number, reason?: string) =>
      request<AdminUser & { disabled_reason?: string | null }>(`/admin/users/${id}/disable`, {
        method: 'POST',
        body: reason ? { reason } : {},
      }),
    enableUser: (id: number) => request<AdminUser>(`/admin/users/${id}/enable`, { method: 'POST' }),
    resetUserPassword: (id: number, newPassword: string) =>
      request<void>(`/admin/users/${id}/reset-password`, {
        method: 'POST',
        body: { new_password: newPassword },
      }),
    verifyUserEmail: (id: number) =>
      request<void>(`/admin/users/${id}/verify-email`, { method: 'POST' }),
    listAgents: (params?: { user_id?: number; q?: string; page?: number; page_size?: number }) =>
      request<ListEnvelope<AdminAgent>>(`/admin/agents${buildQuery(params)}`),
    listDevices: (params?: {
      user_id?: number
      cloud_status?: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
      q?: string
      page?: number
      page_size?: number
    }) => request<ListEnvelope<AdminDevice>>(`/admin/devices${buildQuery(params)}`),
    listWakes: (params?: {
      user_id?: number
      q?: string
      wake_type?: string
      result?: string
      since?: string
      until?: string
      page?: number
      page_size?: number
    }) => request<ListEnvelope<AdminWake>>(`/admin/wakes${buildQuery(params)}`),
    listIntegrations: (params?: {
      user_id?: number
      status?: string
      q?: string
      page?: number
      page_size?: number
    }) => request<ListEnvelope<AdminIntegration>>(`/admin/integrations${buildQuery(params)}`),
    listActivity: (params?: {
      kind?: 'login' | 'audit'
      result?: 'success' | 'failed'
      q?: string
      since?: string
      until?: string
      page?: number
      page_size?: number
    }) => request<ListEnvelope<ActivityItem>>(`/admin/activity${buildQuery(params)}`),
    getMaintenance: () => request<MaintenanceStatus>('/admin/maintenance'),
    setMaintenance: (data: { enabled: boolean; mode: string; message?: string }) =>
      request<MaintenanceStatus>('/admin/maintenance', { method: 'POST', body: data }),
    // 风控运行时控制（admin-risk-controls WRFC）
    getMailer: () => request<MailerControlStatus>('/admin/mailer'),
    setMailer: (data: {
      enabled?: boolean
      limits?: { register: number; resend: number; reset: number }
    }) => request<MailerControlStatus>('/admin/mailer', { method: 'POST', body: data }),
    getPow: () => request<PowStatus>('/admin/pow'),
    setPow: (difficulty: number) =>
      request<PowStatus>('/admin/pow', { method: 'POST', body: { difficulty } }),
    listIpBans: () => request<IpBanEntry[]>('/admin/ip-bans'),
    addIpBan: (data: { target: string; reason?: string; ttl_hours?: number | null }) =>
      request<IpBanEntry>('/admin/ip-bans', { method: 'POST', body: data }),
    removeIpBan: (id: string) => request<void>(`/admin/ip-bans/${id}`, { method: 'DELETE' }),
    risk: () => request<RiskOverview>('/admin/risk'),
    resyncDevice: (did: string) =>
      request<void>(`/admin/devices/${did}/resync`, { method: 'POST' }),
    resyncIntegration: (id: number) =>
      request<void>(`/admin/integrations/${id}/resync`, { method: 'POST' }),
    disconnectAgent: (id: number) =>
      request<void>(`/admin/agents/${id}/disconnect`, { method: 'POST' }),
    auditLog: (params?: { action?: string; page?: number; page_size?: number }) =>
      request<ListEnvelope<AuditLogEntry>>(`/admin/audit-log${buildQuery(params)}`),
  },
  // 公开健康端点（无认证，ui-ux-risk-control §2.4 维护横幅用）。
  health: {
    maintenance: () => request<MaintenanceStatus>('/health/maintenance', { skipAuthRefresh: true }),
  },
}
