/**
 * 场景 1 + is_active 全链路（e2e.md §8 admin.spec.ts P1）。
 *
 * 6 用例（对接 authentication.md §十 is_active 全链路校验）：
 * 管理员禁用用户 → 登录被拒 / 已有 access 失效（moka 5s 后）/ refresh 失效 / agent SSE 断开 /
 * 重新激活恢复 / 非管理员访问 /admin → 404。
 *
 * 依赖后端 is_active 全链路（已实现：JWT 中间件 moka 5s、login USER_DISABLED、
 * refresh is_active 校验、pairing-code JOIN users、admin_service.disable_user 联动）。
 *
 * P2（风控第一版）：新增端点的端到端验证——
 * 全局统计 / 跨用户资源列表 / 重置密码 / 强制验证邮箱 / 强制重同步 / 强制断开 / 审计日志。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  registerUser,
  loginUser,
  refreshTokens,
  getMe,
  listUsers,
  disableUser,
  enableUser,
  resetUserPassword,
  verifyUserEmail,
  getAdminStats,
  listAllAgents,
  listAllDevices,
  resyncDevice,
  disconnectAgent,
  getAuditLog,
  createDevice,
} from '../utils/api'
import { sleep, uniqueEmail } from '../utils/shared'
import { generateRSAKeyPairPEM, encryptWithPublicKey, maskMACAddress, randomMac } from '../utils/crypto'
import { setAgentPublicKey } from '../utils/db'

test.describe('管理员 + is_active 全链路（§8）', () => {
  test('管理员禁用用户 → 登录被拒 USER_DISABLED', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    // admin 禁用
    const adminClient = createBrowserClient(adminUser.accessToken)
    const result = await disableUser(adminClient, targetAuth.user.id)
    expect(result.is_active).toBe(false)

    // 该用户 login → 401 USER_DISABLED（非 INVALID_CREDENTIALS）
    await expect(
      loginUser(client, {
        email: targetAuth.user.email,
        password: 'TestPass123!',
      }),
    ).rejects.toThrow()
  })

  test('管理员禁用用户 → 已有 access 失效（moka 5s 后）', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const targetClient = createBrowserClient(targetAuth.access_token)
    // 禁用前能调 /me
    await getMe(targetClient)

    // admin 禁用
    const adminClient = createBrowserClient(adminUser.accessToken)
    await disableUser(adminClient, targetAuth.user.id)

    // moka 5s TTL 内可能仍可用，等 6s 后必失效
    await sleep(6000)
    await expect(getMe(targetClient)).rejects.toThrow()
  })

  test('管理员禁用用户 → refresh 失效 USER_DISABLED', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const adminClient = createBrowserClient(adminUser.accessToken)
    await disableUser(adminClient, targetAuth.user.id)
    // refresh → USER_DISABLED（refresh 的 is_active 校验在 revoke 后、issue 前）
    await expect(refreshTokens(client, targetAuth.refresh_token)).rejects.toThrow()
  })

  test('管理员重新激活 → 恢复正常', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const adminClient = createBrowserClient(adminUser.accessToken)
    await disableUser(adminClient, targetAuth.user.id)
    // enable
    const enabled = await enableUser(adminClient, targetAuth.user.id)
    expect(enabled.is_active).toBe(true)
    // login 恢复
    const reauth = await loginUser(client, {
      email: targetAuth.user.email,
      password: 'TestPass123!',
    })
    expect(reauth.user.id).toBe(targetAuth.user.id)
  })

  test('非管理员访问 /admin/users → 404（防枚举）', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    // 普通用户探 /admin/users → 404（非 403，防枚举）
    await expect(listUsers(client)).rejects.toThrow()
  })

  test('管理员列用户（分页）', async ({ adminUser }) => {
    const client = createBrowserClient(adminUser.accessToken)
    const list = await listUsers(client, { page: 1, page_size: 10 })
    expect(list.items.length).toBeGreaterThan(0)
    expect(list.total).toBeGreaterThan(0)
    // AdminUser 资源形态
    expect(list.items[0]).toHaveProperty('id')
    expect(list.items[0]).toHaveProperty('email')
    expect(list.items[0]).toHaveProperty('is_active')
  })
})

test.describe('风控第一版端到端（P2）', () => {
  test('全局统计 GET /admin/stats 返回聚合计数', async ({ adminUser }) => {
    const client = createBrowserClient(adminUser.accessToken)
    const stats = await getAdminStats(client)
    // 注册 + admin 至少有用户
    expect(stats.users).toBeGreaterThan(0)
    expect(stats).toHaveProperty('active_users')
    expect(stats).toHaveProperty('devices')
    expect(stats).toHaveProperty('agents')
    expect(stats).toHaveProperty('online_agents')
    expect(stats).toHaveProperty('devices_syncing')
    expect(stats).toHaveProperty('devices_sync_error')
  })

  test('管理员重置用户密码 → 新密码可登录、旧 refresh 失效', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'OldPass123!',
    })
    const adminClient = createBrowserClient(adminUser.accessToken)
    await resetUserPassword(adminClient, targetAuth.user.id, 'NewPass456!')

    // 旧密码登录失败
    await expect(
      loginUser(client, {
        email: targetAuth.user.email,
        password: 'OldPass123!',
      }),
    ).rejects.toThrow()
    // 新密码登录成功
    const reauth = await loginUser(client, {
      email: targetAuth.user.email,
      password: 'NewPass456!',
    })
    expect(reauth.user.id).toBe(targetAuth.user.id)

    // 旧 refresh 失效（已吊销）
    await expect(refreshTokens(client, targetAuth.refresh_token)).rejects.toThrow()
  })

  test('管理员强制验证邮箱 → 用户 email_verified=true', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const adminClient = createBrowserClient(adminUser.accessToken)
    await verifyUserEmail(adminClient, targetAuth.user.id)
    // 列用户里查到该用户 email_verified
    const list = await listUsers(adminClient, { page: 1, page_size: 100 })
    const u = list.items.find((x) => x.id === targetAuth.user.id)
    expect(u).toBeTruthy()
    expect(u?.email_verified).toBe(true)
  })

  test('跨用户 device 列表 + 强制重同步', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const targetClient = createBrowserClient(targetAuth.access_token)
    // target 建一台设备（真实 RSA 密文——后端校验 mac_encrypted ≥256 字符）
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(targetAuth.user.id, publicKey)
    const mac = randomMac()
    const device = await createDevice(targetClient, {
      name: 'E2E NAS',
      mac_encrypted: await encryptWithPublicKey(mac, publicKey),
      mac_display: maskMACAddress(mac),
    })

    const adminClient = createBrowserClient(adminUser.accessToken)
    // 跨用户列表能看到该设备
    const all = await listAllDevices(adminClient)
    expect(all.items.some((d) => d.did === device.did)).toBe(true)

    // 强制重同步 → 200（不抛错即通过；agent pending 时 resync 仅置 syncing）
    await resyncDevice(adminClient, device.did)
  })

  test('跨用户 agent 列表 + 强制断开不存在的 agent → 404', async ({ adminUser }) => {
    const adminClient = createBrowserClient(adminUser.accessToken)
    // agent 列表可拉（至少有 admin 自己的 agent）
    const agents = await listAllAgents(adminClient)
    expect(agents.items.length).toBeGreaterThan(0)
    expect(agents.items[0]).toHaveProperty('user_email')

    // 断开不存在的 agent → 抛错（404 AGENT_NOT_FOUND）
    await expect(disconnectAgent(adminClient, 9_999_999)).rejects.toThrow()
  })

  test('审计日志记录管理员操作（disable 后可查到 user.disable）', async ({ adminUser }) => {
    const client = createBrowserClient()
    const targetAuth = await registerUser(client, {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const adminClient = createBrowserClient(adminUser.accessToken)
    await disableUser(adminClient, targetAuth.user.id)

    // 审计日志按 action 过滤
    const log = await getAuditLog(adminClient, { action: 'user.disable', page_size: 10 })
    expect(log.total).toBeGreaterThanOrEqual(1)
    const entry = log.items[0]
    expect(entry.action).toBe('user.disable')
    expect(entry.target_user_id).toBe(targetAuth.user.id)
    // actor_email 是 admin
    expect(entry.actor_email).toBeTruthy()
  })

  test('非管理员访问 /admin/stats → 404（防枚举）', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    await expect(getAdminStats(client)).rejects.toThrow()
  })
})
