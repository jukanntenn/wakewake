/**
 * 密码重置 E2E（e2e.md §9）。
 *
 * 2 用例：请求重置 → 收邮件 → confirm / 不存在邮箱也返 202（防枚举）。
 * 用 mailpit 容器（profiles: [mail]）mock SMTP。
 *
 * PoW 自动绕过：mailer=true 时 PoW 校验执行，但需传合法 challenge+nonce。
 * 简化：用 GET /pow/challenge 拿 challenge，本地算 nonce（SHA-256 前导零）。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  registerUser,
  requestPasswordReset,
  confirmPasswordReset,
  getPowChallenge,
  refreshTokens,
} from '../utils/api'
import { execSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { uniqueEmail } from '../utils/shared'

const MAILPIT_CONTAINER = 'e2e-mailpit-1'

/** 本地解 PoW（SHA-256 前导零，authentication.md §六）。 */
function solvePow(challenge: string, difficulty: number): string {
  const target = '0'.repeat(difficulty)
  for (let nonce = 0; ; nonce++) {
    const hash = createHash('sha256').update(challenge + nonce).digest('hex')
    if (hash.startsWith(target)) return nonce.toString()
  }
}

/** 从 mailpit 取最新邮件，提取 reset token。 */
function extractResetTokenFromMailpit(toEmail: string): string | null {
  const raw = execSync(
    `docker exec ${MAILPIT_CONTAINER} sh -c "wget -qO- http://localhost:8025/api/v1/messages"`,
  ).toString()
  const messages = JSON.parse(raw)
  const msg = messages.messages?.find((m: { To?: { Address: string }[] }) =>
    m.To?.some((t) => t.Address === toEmail),
  )
  if (!msg) return null
  // 取邮件 body（mailpit API 字段是 Text/HTML，非 Body）
  const body = execSync(
    `docker exec ${MAILPIT_CONTAINER} sh -c "wget -qO- http://localhost:8025/api/v1/message/${msg.ID}"`,
  ).toString()
  const parsed = JSON.parse(body)
  const text = parsed.Text || parsed.HTML || parsed.Body || ''
  // token 在链接里（?token=...）
  const match = text.match(/token=([^&\s"']+)/)
  return match ? match[1] : null
}

test.describe('密码重置（§9，@mail）', () => {
  test('请求重置 → 收邮件 → confirm 改密成功', async ({}) => {
    const client = createBrowserClient()
    const email = uniqueEmail()
    await registerUser(client, { email, password: 'OldPass123!' })

    // PoW
    const challenge = await getPowChallenge(client)
    const nonce = solvePow(challenge.challenge, challenge.difficulty)

    // 请求重置（202）。challenge 字段传 challenge.id（UUID，server pow.verify 按此查找）。
    await requestPasswordReset(client, {
      email,
      challenge: challenge.id,
      nonce,
    })

    // 等 mailpit 收邮件
    let token: string | null = null
    for (let i = 0; i < 30; i++) {
      token = extractResetTokenFromMailpit(email)
      if (token) break
      await new Promise((r) => setTimeout(r, 1000))
    }
    expect(token).toBeTruthy()

    // confirm 改密
    const result = await confirmPasswordReset(client, {
      token: token!,
      new_password: 'ResetNew123!',
    })
    expect(result.access_token).toBeTruthy()
    expect(result.refresh_token).toBeTruthy()
  })

  test('不存在邮箱也返 202（防枚举）', async ({}) => {
    const client = createBrowserClient()
    const challenge = await getPowChallenge(client)
    const nonce = solvePow(challenge.challenge, challenge.difficulty)
    // 不存在的邮箱 → 仍 202（不泄露存在性）。challenge 字段传 challenge.id（UUID）。
    await expect(
      requestPasswordReset(client, {
        email: 'nonexistent-' + uniqueEmail(),
        challenge: challenge.id,
        nonce,
      }),
    ).resolves.toBeUndefined()
  })
})
