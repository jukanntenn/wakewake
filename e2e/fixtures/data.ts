/**
 * 三层 fixture 继承链 第 2 层：数据 fixture（e2e.md §3.4/§4.4）。
 *
 * pom.ts base
 *    ↓ extend<DataFixtures>（data.ts：ky 准备数据）
 */
import { base as pomBase, expect } from './pom'
import {
  createBrowserClient,
  registerUser,
  loginUser,
  getDefaultAgent,
  rotatePairingCode,
  createDevice,
  createIntegration,
  type AuthResponse,
  type DefaultAgent,
  type Device,
  type Integration,
} from '../utils/api'
import {
  generateRSAKeyPairPEM,
  encryptWithPublicKey,
  maskMACAddress,
  randomMac,
  type RSAKeyPairPEM,
} from '../utils/crypto'
import { execSql, setAgentPublicKey } from '../utils/db'
import { AgentContainer, AGENT_CONTAINER } from '../utils/agent'
import { uniqueEmail } from '../utils/shared'

/** 注册用户 fixture 数据形态（e2e.md §4.4）。 */
export interface UserFixture {
  user: AuthResponse['user']
  accessToken: string
  refreshToken: string
  email: string
  password: string
}

/** pairedAgent fixture 数据形态。 */
export interface PairedAgentFixture extends UserFixture {
  agent: DefaultAgent
  pairingCode: string
  publicKeyPEM: string
  container: AgentContainer
}

/** staticPublicKey fixture（场景 1 不启 agent，e2e.md §7.2）。 */
export interface StaticPublicKeyFixture {
  publicKeyPEM: string
  privateKeyPEM: string
}

const PASSWORD = 'TestPass123!'

export const base = pomBase.extend<
  // test-scope fixtures（T）
  {
    /** 每测试独立新建用户（配额/跨用户场景，test scope）。 */
    freshUser: UserFixture
    /** 用户工厂（带 superuser 选项）。 */
    createUser: (opts?: { superuser?: boolean }) => Promise<UserFixture>
    /** 预生成 RSA 密钥对（场景 1 staticPublicKey，不启 agent）。 */
    staticPublicKey: StaticPublicKeyFixture
    /** 已配对 agent（test scope，每测试独立 freshUser + 启停，避免状态污染）。 */
    pairedAgent: PairedAgentFixture
    /** 基础设备（场景 1 用 staticPublicKey）。 */
    basicDevice: { device: Device; mac: string }
    /** bemfa 集成。 */
    bemfaIntegration: { integration: Integration }
  },
  // worker-scope fixtures（W）
  {
    /** globalSetup 注册的测试用户（单例，worker scope）。 */
    registeredUser: UserFixture
  }
>({
  // globalSetup 注册的用户：从 storageState 反解 token（globalSetup 已写）
  registeredUser: [
    async ({}, use) => {
      // globalSetup 已注册 e2e@wakewake.local，这里重新登录拿新 token
      // （storageState 的 token 可能过期，login 拿 fresh）
      const client = createBrowserClient()
      const email = process.env.E2E_USER_EMAIL ?? 'e2e@wakewake.local'
      const password = process.env.E2E_USER_PASSWORD ?? 'TestPass123!'
      const auth = await loginUser(client, { email, password })
      await use({
        user: auth.user,
        accessToken: auth.access_token,
        refreshToken: auth.refresh_token,
        email,
        password,
      })
    },
    { scope: 'worker' },
  ],

  // 每测试独立新建用户（配额/跨用户场景）
  freshUser: async ({}, use) => {
    const client = createBrowserClient()
    const email = uniqueEmail()
    const auth = await registerUser(client, { email, password: PASSWORD })
    await use({
      user: auth.user,
      accessToken: auth.access_token,
      refreshToken: auth.refresh_token,
      email,
      password: PASSWORD,
    })
  },

  // 用户工厂
  createUser: async ({}, use) => {
    await use(async (opts?: { superuser?: boolean }) => {
      const client = createBrowserClient()
      const email = uniqueEmail()
      const auth = await registerUser(client, { email, password: PASSWORD })
      if (opts?.superuser) {
        execSql(`UPDATE users SET is_superuser = true WHERE id = ${auth.user.id};`)
        // 重新登录拿带 is_superuser=true 的 access
        const reauth = await loginUser(client, { email, password: PASSWORD })
        return {
          user: reauth.user,
          accessToken: reauth.access_token,
          refreshToken: reauth.refresh_token,
          email,
          password: PASSWORD,
        }
      }
      return {
        user: auth.user,
        accessToken: auth.access_token,
        refreshToken: auth.refresh_token,
        email,
        password: PASSWORD,
      }
    })
  },

  // 预生成 RSA 密钥对（场景 1 不启 agent，DB 直改 agent.public_key）
  staticPublicKey: async ({ freshUser }, use) => {
    const { publicKey, privateKey } = generateRSAKeyPairPEM()
    // DB 直改 agent.public_key（让该用户 agent 状态从 pending → offline）
    setAgentPublicKey(freshUser.user.id, publicKey)
    await use({ publicKeyPEM: publicKey, privateKeyPEM: privateKey })
  },

  // pairedAgent：真实 agent 容器（worker scope，e2e.md §6.4）。
  // 依赖 registeredUser（worker scope 一致）。CI workers=1，本地多 worker 时每 worker
  // 用不同 pairing_code（globalSetup 预建多个测试用户，未来增强）。
  // pairedAgent：test scope（每测试独立 freshUser + 启停 agent，避免状态污染）。
  // 场景 2/3 测试间状态敏感（device_add 改 sync_status、wake 写 wakes 表），
  // 用 freshUser 保证每测试干净。CI workers=1 串行跑。
  pairedAgent: async ({ freshUser }, use) => {
    const browserClient = createBrowserClient(freshUser.accessToken)
    // freshUser 的 agent 是 pending（无 public_key），pairing_code 完整
    const agent = await getDefaultAgent(browserClient)
    if (!agent.pairing_code || agent.pairing_code.includes('*')) {
      throw new Error('freshUser agent should be pending (full pairing_code)')
    }
    const container = new AgentContainer(AGENT_CONTAINER, agent.pairing_code)
    await container.start(browserClient)
    const onlineAgent = await getDefaultAgent(browserClient)
    await use({
      ...freshUser,
      agent: onlineAgent,
      pairingCode: agent.pairing_code,
      publicKeyPEM: onlineAgent.public_key ?? '',
      container,
    })
    await container.stop()
  },

  // 基础设备（用 staticPublicKey 加密 MAC，API 创建）
  basicDevice: async ({ freshUser, staticPublicKey }, use) => {
    const client = createBrowserClient(freshUser.accessToken)
    const mac = randomMac()
    const macEncrypted = await encryptWithPublicKey(mac, staticPublicKey.publicKeyPEM)
    const device = await createDevice(client, {
      name: `Test Device ${Date.now()}`,
      mac_encrypted: macEncrypted,
      mac_display: maskMACAddress(mac),
    })
    await use({ device, mac })
  },

  // bemfa 集成（用 staticPublicKey 加密 uid）
  bemfaIntegration: async ({ freshUser, staticPublicKey }, use) => {
    const client = createBrowserClient(freshUser.accessToken)
    const uidEncrypted = await encryptWithPublicKey(
      'test-bemfa-uid',
      staticPublicKey.publicKeyPEM,
    )
    const integration = await createIntegration(client, 'bemfa', {
      uid: uidEncrypted,
      topic_prefix: `e2e_${Date.now()}`,
    })
    await use({ integration })
  },
})

export { AGENT_CONTAINER, expect }
export type { RSAKeyPairPEM }
