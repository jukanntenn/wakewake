/**
 * 场景 3：用户↔server↔agent —— 配对（e2e.md §7.4 pairing.spec.ts P0）。
 *
 * 1 用例：完整配对流程（freshUser → 启 agent → 浏览器看 agents 页）。
 */
import { test, expect } from '../fixtures'
import { createBrowserClient, getDefaultAgent } from '../utils/api'
import { AgentContainer, AGENT_CONTAINER } from '../utils/agent'
import { uniqueEmail } from '../utils/shared'
import { registerUser } from '../utils/api'

test.describe('完整配对流程（场景 3）', () => {
  test('freshUser → 启 agent → online', async ({ page }) => {
    // 独立 freshUser（不污染 worker-scoped registeredUser）
    const email = uniqueEmail()
    const auth = await registerUser(createBrowserClient(), {
      email,
      password: 'TestPass123!',
    })
    const client = createBrowserClient(auth.access_token)

    // 1. pending 状态（pairing_code 完整）
    let agent = await getDefaultAgent(client)
    expect(agent.status).toBe('pending')
    expect(agent.pairing_code).not.toContain('*')

    // 2. 启 agent 容器
    const container = new AgentContainer(AGENT_CONTAINER, agent.pairing_code)
    await container.start(client)

    // 3. 浏览器看 agents 页 → pending→online（绿点）
    await page.goto('/dashboard/agents')
    await page.waitForLoadState('networkidle')
    agent = await getDefaultAgent(client)
    expect(agent.status).toBe('online')
    // online 后 pairing_code 脱敏
    expect(agent.pairing_code).toContain('*')

    await container.stop()
  })
})
