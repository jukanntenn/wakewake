/**
 * 风控运行时控制 E2E（admin-risk-controls WRFC）。
 *
 * 覆盖：mailer 总闸（关 → 重置 503 → 恢复）、重置分路预算耗尽 → 503、
 * PoW 难度旋钮（新 challenge 生效）、IP 封禁往返（精确 IP + CIDR）、
 * 自封守卫（0.0.0.0/0 必覆盖请求者 → 422）、非 admin 访问 → 404、
 * 维护态注册联动（API 403 + 注册页 UI 禁用）。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  registerUser,
  requestPasswordReset,
  getPowChallenge,
  solvePow,
  getMailer,
  setMailer,
  getPow,
  setPow,
  listIpBans,
  addIpBan,
  removeIpBan,
  getRisk,
  setMaintenance,
} from '../utils/api'
import { uniqueEmail } from '../utils/shared'

test.describe('风控运行时控制（admin-risk-controls）', () => {
  test('mailer 总闸：运行时关闭 → 重置请求 503 → 恢复后正常', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    const client = createBrowserClient()

    // 关闸（记原状态，用后恢复）
    const before = await getMailer(admin)
    expect(before.enabled).toBe(true)
    const off = await setMailer(admin, { enabled: false })
    expect(off.enabled).toBe(false)

    // 重置请求 → 503（与 mailer.enabled=false 同语义）
    const challenge = await getPowChallenge(client)
    const nonce = solvePow(challenge.challenge, challenge.difficulty)
    await expect(
      requestPasswordReset(client, { email: uniqueEmail(), challenge: challenge.id, nonce }),
    ).rejects.toThrow()

    // risk 面板同步可见
    const risk = await getRisk(admin)
    expect(risk.mailer.enabled).toBe(false)

    // 恢复
    const on = await setMailer(admin, { enabled: true })
    expect(on.enabled).toBe(true)
  })

  test('重置分路预算耗尽 → 503（注册洪水烧不干份额的机制验证）', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    const client = createBrowserClient()
    const original = await getMailer(admin)

    // 预算按「当日已发送数」判定：压到 当前已用 + 1，使下一封恰好可用、再下一封 503。
    const limit = original.sent.reset + 1
    await setMailer(admin, {
      limits: { ...original.limits, reset: limit },
    })

    try {
      // 发送必须命中真实账号（防枚举：不存在邮箱静默跳过、不消耗预算）。
      const target = await registerUser(client, {
        email: uniqueEmail(),
        password: 'TestPass123!',
      })
      const c1 = await getPowChallenge(client)
      await requestPasswordReset(client, {
        email: target.user.email,
        challenge: c1.id,
        nonce: solvePow(c1.challenge, c1.difficulty),
      })

      // 路由预检 would_send=false → 503
      const c2 = await getPowChallenge(client)
      await expect(
        requestPasswordReset(client, {
          email: target.user.email,
          challenge: c2.id,
          nonce: solvePow(c2.challenge, c2.difficulty),
        }),
      ).rejects.toThrow()

      // blocked 计数可见（拒绝不静默）
      const snap = await getMailer(admin)
      expect(snap.sent.reset).toBe(limit)
      expect(snap.blocked.reset).toBeGreaterThanOrEqual(1)
    } finally {
      await setMailer(admin, { limits: original.limits })
    }
  })

  test('注册预算耗尽：注册仍成功（静默降级）且拒绝可见', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    const client = createBrowserClient()
    const original = await getMailer(admin)

    // 把注册预算压到当前已用 → 下一次注册的验证邮件必被拒
    await setMailer(admin, {
      limits: { ...original.limits, register: original.sent.register },
    })

    try {
      const email = uniqueEmail()
      const password = 'TestPass123!'
      const ch = await getPowChallenge(client)
      // 直接调 register（不经 registerUser：未验证账号无法登录，恰好验证
      // 「预算耗尽时注册照常、账号停留未验证」的降级语义）
      const user = await client
        .post('auth/register', {
          json: { email, password, challenge: ch.id, nonce: solvePow(ch.challenge, ch.difficulty) },
        })
        .json<{ id: number; email_verified?: boolean }>()
      expect(user.id).toBeGreaterThan(0)

      const snap = await getMailer(admin)
      expect(snap.sent.register).toBe(original.sent.register)
      expect(snap.blocked.register).toBeGreaterThanOrEqual(1)

      // 未验证堆积在 risk 面板可见
      const risk = await getRisk(admin)
      expect(risk.unverified_count).toBeGreaterThanOrEqual(1)
    } finally {
      await setMailer(admin, { limits: original.limits })
    }
  })

  test('PoW 难度旋钮：上调对新 challenge 生效', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    const original = await getPow(admin)

    await setPow(admin, 6)
    try {
      const pow = await getPow(admin)
      expect(pow.difficulty).toBe(6)
      const client = createBrowserClient()
      const challenge = await getPowChallenge(client)
      expect(challenge.difficulty).toBe(6)
    } finally {
      await setPow(admin, original.difficulty)
    }
  })

  test('IP 封禁往返：精确 IP 与 CIDR，移除后消失', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)

    const exact = await addIpBan(admin, {
      target: '203.0.113.77',
      reason: 'e2e exact',
      ttl_hours: 1,
    })
    expect(exact.kind).toBe('ip')

    const cidr = await addIpBan(admin, {
      target: '198.51.100.0/24',
      reason: 'e2e cidr',
      ttl_hours: 1,
    })
    expect(cidr.kind).toBe('cidr')

    const list = await listIpBans(admin)
    expect(list.some((b) => b.id === exact.id)).toBe(true)
    expect(list.some((b) => b.id === cidr.id)).toBe(true)

    await removeIpBan(admin, exact.id)
    await removeIpBan(admin, cidr.id)
    const after = await listIpBans(admin)
    expect(after.some((b) => b.id === exact.id || b.id === cidr.id)).toBe(false)
  })

  test('自封守卫：0.0.0.0/0 覆盖请求者自身 → 422', async ({ adminUser }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    await expect(addIpBan(admin, { target: '0.0.0.0/0', ttl_hours: 1 })).rejects.toThrow()
    // 未留下任何条目
    const list = await listIpBans(admin)
    expect(list.some((b) => b.target === '0.0.0.0/0')).toBe(false)
  })

  test('非管理员访问 /admin/ip-bans → 404（防枚举）', async ({}) => {
    const userAuth = await registerUser(createBrowserClient(), {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const userClient = createBrowserClient(userAuth.access_token)
    await expect(listIpBans(userClient)).rejects.toThrow()
  })

  test('维护态注册联动：API 403 + 注册页 UI 禁用', async ({ adminUser, page }) => {
    const admin = createBrowserClient(adminUser.accessToken)
    const client = createBrowserClient()

    // 开启 registration_disabled（带消息，注册页优先展示）
    await setMaintenance(admin, {
      enabled: true,
      mode: 'registration_disabled',
      message: 'e2e registration closed',
    })

    // 项目无关的未登录页面：chromium/admin project 带 storageState 登录态，
    // PublicRoute 会重定向——清掉 auth storage 再 reload 即得未访客视角。
    const gotoRegisterAsVisitor = async () => {
      await page.goto('/register')
      await page.evaluate(() => localStorage.removeItem('wakewake-auth'))
      await page.reload()
    }

    try {
      // API：注册被拦（maintenance 中间件先于 PoW 拒绝）
      await expect(
        registerUser(client, { email: uniqueEmail(), password: 'TestPass123!' }),
      ).rejects.toThrow()

      // UI：注册页主动禁用表单并显示公告（轮询 /health/maintenance）
      await gotoRegisterAsVisitor()
      await expect(page.getByText('e2e registration closed')).toBeVisible({ timeout: 15_000 })
      await expect(page.getByRole('button', { name: /sign.?up|register/i })).toBeDisabled()
    } finally {
      await setMaintenance(admin, { enabled: false, mode: 'registration_disabled' })
    }

    // 恢复后：注册页表单可用
    await gotoRegisterAsVisitor()
    await expect(page.getByRole('button', { name: /sign.?up|register/i })).toBeEnabled({
      timeout: 15_000,
    })
  })
})
