/**
 * ky HTTP 封装 + 业务 API 函数（e2e.md §4.1-4.2）。
 *
 * ⚠️ ky v2.0.2 API 修正（vs e2e.md 示例，以 .local/contexts/ky 源码为准）：
 *   - `prefixUrl` 已重命名为 `prefix`（v2 用 prefixUrl 会 throw，source/core/Ky.ts:349-351）。
 *   - hooks.beforeRequest 签名改为单一 state 对象 `({request, options, retryCount}) => ...`
 *     （source/types/hooks.ts:30-40），非 v1 的 `(request, options) => ...`。
 *   - retry.methods 是可变数组（HttpMethod[]），不能用 `as const`。
 *
 * 两套认证域（api-design.md §0.2）→ 两套 ky 客户端工厂：
 *   - 浏览器域（JWT）：createBrowserClient(accessToken?)
 *   - Agent 域（pairing_code）：createAgentClient(pairingCode)
 */
import ky, { type KyInstance, type BeforeRequestHook } from 'ky'
import { execSql } from './db'

const BASE_URL = process.env.BASE_URL ?? 'https://localhost:8443'

/** 公共重试配置（e2e.md §4.1）。 */
const COMMON_RETRY = {
  limit: 3,
  methods: ['get', 'post', 'patch', 'delete'] as string[],
  statusCodes: [408, 429, 500, 502, 503, 504],
}

// ---- 类型（对应 api-design.md §1.4 端点表）----

export interface AuthResponse {
  access_token: string
  refresh_token: string
  expires_in: number
  user: User
}

export interface User {
  id: number
  email: string
  is_superuser: boolean
}

export interface Device {
  did: string
  name: string
  mac_display: string
  description?: string
  // device-sync-v3 §8.3 派生字段
  agent_online: boolean
  projection_status: 'syncing' | 'synced' | 'agent_offline'
  cloud_status: 'not_observed' | 'syncing' | 'synced' | 'error' | 'no_integration'
  cloud_observed_name: string | null
  cloud_observed_at: string | null
  last_drift_at: string | null
  last_drift_kind: 'deleted' | 'renamed' | null
  last_error: string | null
  agent_id?: string
  created_at: string
  updated_at: string
}

export interface Command {
  command_id: string
  type: string
  status: 'pending' | 'dispatched' | 'completed' | 'failed' | 'expired'
  result?: { success: boolean; message: string }
  device_id?: string
  created_at: string
  completed_at?: string
}

export interface DefaultAgent {
  aid: string
  name: string
  status: 'pending' | 'online' | 'offline'
  pairing_code: string // pending 完整，online/offline 脱敏
  public_key: string | null
  last_seen: string | null
  created_at: string
}

export interface Integration {
  provider: string
  config: Record<string, unknown>
  enabled: boolean
  // device-sync-v3 §8.5 7 态
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
  last_report_at?: string
  created_at: string
  updated_at: string
}

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

export interface Wake {
  id: number
  device_id: string
  device_name: string
  type: 'wol' | 'bemfa_wake'
  status: 'success' | 'failed' | 'expired'
  message: string
  created_at: string
}

export interface PaginatedList<T> {
  items: T[]
  page?: number
  page_size?: number
  total?: number
}

// ---- 两套客户端工厂（e2e.md §4.1）----

/**
 * 浏览器域客户端（JWT）。不做 401 自动 refresh（e2e.md §5.4）。
 * 生产 TTL(15min) 下 ky 调用不该 401；若 401 视为错误（测试失败信号）。
 */
export function createBrowserClient(accessToken?: string): KyInstance {
  // ky v2：beforeRequest hook 接收单一 state 对象（非 v1 的 (req, options)）
  const beforeRequest: BeforeRequestHook[] = accessToken
    ? [
        ({ request }) => {
          request.headers.set('Authorization', `Bearer ${accessToken}`)
        },
      ]
    : []
  return ky.create({
    prefix: `${BASE_URL}/api/v1`, // ky v2：prefix（非 prefixUrl）
    timeout: 30_000,
    retry: COMMON_RETRY,
    hooks: { beforeRequest },
  })
}

/** Agent 域客户端（pairing_code）。 */
export function createAgentClient(pairingCode: string): KyInstance {
  return ky.create({
    prefix: `${BASE_URL}/api/v1/agents/self`,
    timeout: 60_000,
    retry: { ...COMMON_RETRY, methods: ['get', 'post'] as string[] },
    hooks: {
      beforeRequest: [
        ({ request }) => {
          request.headers.set('Authorization', `Bearer ${pairingCode}`)
        },
      ],
    },
  })
}

// ---- 业务封装函数（e2e.md §4.2 端点全表）----

// 认证（公开）
// 注册流程（redesign 后）：POST /auth/register 只建用户（email_verified=false，无 token）；
// 需邮件验证后才能 login。E2E 用 DB 直改 email_verified（e2e.md §4.5 已有先例），
// 再 login 拿 token——保持 registerUser 调用方拿到可用 AuthResponse 的语义。
export async function registerUser(
  client: KyInstance,
  input: { email: string; password: string },
): Promise<AuthResponse> {
  const user = await client
    .post('auth/register', { json: input })
    .json<{ id: number; email: string }>()
  execSql(`UPDATE users SET email_verified = true WHERE id = ${user.id};`)
  return loginUser(client, input)
}

export async function loginUser(
  client: KyInstance,
  input: { email: string; password: string },
): Promise<AuthResponse> {
  return client.post('auth/login', { json: input }).json()
}

export async function refreshTokens(
  client: KyInstance,
  refreshToken: string,
): Promise<{ access_token: string; refresh_token: string; expires_in: number }> {
  return client.post('auth/refresh', { json: { refresh_token: refreshToken } }).json()
}

export async function logout(
  client: KyInstance,
  refreshToken: string,
): Promise<void> {
  await client.post('me/logout', { json: { refresh_token: refreshToken } })
}

export async function requestPasswordReset(
  client: KyInstance,
  input: { email: string; challenge: string; nonce: string },
): Promise<void> {
  // 恒 202（防枚举）
  await client.post('auth/password-reset/request', { json: input })
}

export async function confirmPasswordReset(
  client: KyInstance,
  input: { token: string; new_password: string },
): Promise<{ access_token: string; refresh_token: string; expires_in: number }> {
  return client.post('auth/password-reset/confirm', { json: input }).json()
}

export async function getPowChallenge(
  client: KyInstance,
): Promise<{ id: string; challenge: string; difficulty: number }> {
  return client.get('pow/challenge').json()
}

// 用户（JWT）
export async function getMe(client: KyInstance): Promise<User> {
  return client.get('me').json()
}

export async function changePassword(
  client: KyInstance,
  input: { current_password: string; new_password: string },
): Promise<void> {
  await client.post('me/password', { json: input })
}

// 设备（JWT）
export async function listDevices(client: KyInstance): Promise<Device[]> {
  const res = await client.get('devices').json<Device[] | PaginatedList<Device>>()
  return Array.isArray(res) ? res : res.items
}

export async function createDevice(
  client: KyInstance,
  input: {
    name: string
    mac_encrypted: string
    mac_display: string
    description?: string
    agent_id?: string
  },
): Promise<Device> {
  return client.post('devices', { json: input }).json()
}

export async function getDevice(client: KyInstance, did: string): Promise<Device> {
  return client.get(`devices/${did}`).json()
}

export async function updateDevice(
  client: KyInstance,
  did: string,
  patch: Partial<Device>,
): Promise<Device> {
  return client.patch(`devices/${did}`, { json: patch }).json()
}

export async function deleteDevice(
  client: KyInstance,
  did: string,
): Promise<void> {
  await client.delete(`devices/${did}`)
}

export async function wakeDevice(
  client: KyInstance,
  did: string,
): Promise<Command> {
  return client.post(`devices/${did}/wake`).json()
}

export async function getCommand(
  client: KyInstance,
  commandId: string,
): Promise<Command> {
  return client.get(`commands/${commandId}`).json()
}

// Agent（JWT 管理视角）
export async function getDefaultAgent(client: KyInstance): Promise<DefaultAgent> {
  return client.get('agents/default').json()
}

export async function rotatePairingCode(
  client: KyInstance,
): Promise<DefaultAgent> {
  return client.post('agents/default/pairing-code/rotate').json()
}

// 集成（JWT）
export async function listIntegrations(client: KyInstance): Promise<Integration[]> {
  const res = await client
    .get('integrations')
    .json<Integration[] | PaginatedList<Integration>>()
  return Array.isArray(res) ? res : res.items
}

export async function getIntegrationSchema(
  client: KyInstance,
  provider: string,
): Promise<Record<string, unknown>> {
  return client.get(`integrations/${provider}/schema`).json()
}

export async function getIntegration(
  client: KyInstance,
  provider: string,
): Promise<Integration> {
  return client.get(`integrations/${provider}`).json()
}

export async function createIntegration(
  client: KyInstance,
  provider: string,
  config: Record<string, unknown>,
): Promise<Integration> {
  return client.post('integrations', { json: { provider, config } }).json()
}

export async function updateIntegration(
  client: KyInstance,
  provider: string,
  config: Record<string, unknown>,
): Promise<Integration> {
  return client.patch(`integrations/${provider}`, { json: { config } }).json()
}

export async function deleteIntegration(
  client: KyInstance,
  provider: string,
): Promise<void> {
  await client.delete(`integrations/${provider}`)
}

export async function enableIntegration(
  client: KyInstance,
  provider: string,
): Promise<Integration> {
  // 后端 disable/enable 返 200 无 body（实现现状，api-design.md §1.4 规划返 Integration）。
  // E2E 不阻塞：调完用 getIntegration 再查状态。
  await client.post(`integrations/${provider}/enable`)
  // 重新查 integration 状态（enabled=true）
  return client.get(`integrations/${provider}`).json()
}

export async function disableIntegration(
  client: KyInstance,
  provider: string,
): Promise<Integration> {
  await client.post(`integrations/${provider}/disable`)
  return client.get(`integrations/${provider}`).json()
}

// 管理员（JWT + is_superuser）
export async function listUsers(
  client: KyInstance,
  params?: { is_active?: boolean; page?: number; page_size?: number },
): Promise<PaginatedList<AdminUser>> {
  return client.get('admin/users', { searchParams: params ?? {} }).json()
}

export async function disableUser(
  client: KyInstance,
  userId: number,
): Promise<AdminUser> {
  return client.post(`admin/users/${userId}/disable`).json()
}

export async function enableUser(
  client: KyInstance,
  userId: number,
): Promise<AdminUser> {
  return client.post(`admin/users/${userId}/enable`).json()
}

// 管理员重置用户密码（绕过旧密码 + 吊销所有 refresh）
// 后端返 200 空 body（routes/admin.rs reset_password → StatusCode::OK），不调 .json()。
export async function resetUserPassword(
  client: KyInstance,
  userId: number,
  newPassword: string,
): Promise<void> {
  await client
    .post(`admin/users/${userId}/reset-password`, { json: { new_password: newPassword } })
}

// 强制标记邮箱已验证（后端返 200 空 body，同 reset-password）
export async function verifyUserEmail(
  client: KyInstance,
  userId: number,
): Promise<void> {
  await client.post(`admin/users/${userId}/verify-email`)
}

// Admin 全局统计
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
export async function getAdminStats(client: KyInstance): Promise<AdminStats> {
  return client.get('admin/stats').json()
}

// 跨用户 agent 列表
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
export async function listAllAgents(
  client: KyInstance,
  params?: { user_id?: number; page?: number; page_size?: number },
): Promise<PaginatedList<AdminAgent>> {
  return client.get('admin/agents', { searchParams: params ?? {} }).json()
}

// 跨用户 device 列表
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
export async function listAllDevices(
  client: KyInstance,
  params?: {
    user_id?: number
    cloud_status?: string
    page?: number
    page_size?: number
  },
): Promise<PaginatedList<AdminDevice>> {
  return client.get('admin/devices', { searchParams: params ?? {} }).json()
}

// 强制重同步设备（后端返 200 空 body，同 reset-password）
export async function resyncDevice(
  client: KyInstance,
  did: string,
): Promise<void> {
  await client.post(`admin/devices/${did}/resync`)
}

// 强制断开 agent SSE（后端返 200 空 body）
export async function disconnectAgent(
  client: KyInstance,
  agentId: number,
): Promise<void> {
  await client.post(`admin/agents/${agentId}/disconnect`)
}

// 审计日志
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
export async function getAuditLog(
  client: KyInstance,
  params?: { action?: string; page?: number; page_size?: number },
): Promise<PaginatedList<AuditLogEntry>> {
  return client.get('admin/audit-log', { searchParams: params ?? {} }).json()
}

// 唤醒历史（JWT）
export async function listWakes(
  client: KyInstance,
  params?: { device_id?: string; type?: string; before?: string; page_size?: number },
): Promise<PaginatedList<Wake>> {
  return client.get('wakes', { searchParams: params ?? {} }).json()
}
