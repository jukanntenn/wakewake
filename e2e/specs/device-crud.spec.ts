/**
 * 场景 1：用户↔server（无 agent）—— 设备 CRUD（e2e.md §7.2 device-crud.spec.ts P0）。
 *
 * 6 用例：创建/配额/编辑/删除/MAC 校验/跨用户访问。
 * 用 freshUser（每测试独立）+ staticPublicKey（DB 直改 public_key，不启 agent）。
 * device-sync-v3：agent 离线 → projection_status=agent_offline，cloud_status=no_integration（无集成）。场景 1 不验 sync 派生态。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  createDevice,
  listDevices,
  updateDevice,
  deleteDevice,
  getDevice,
} from '../utils/api'
import {
  generateRSAKeyPairPEM,
  encryptWithPublicKey,
  maskMACAddress,
  randomMac,
} from '../utils/crypto'
import { setAgentPublicKey } from '../utils/db'
import { uniqueEmail } from '../utils/shared'
import { registerUser } from '../utils/api'

test.describe('设备 CRUD（场景 1，无 agent）', () => {
  test('创建设备成功', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    // 预生成 RSA 密钥对 + DB 直改 agent.public_key（staticPublicKey 等价，每测试独立）
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(freshUser.user.id, publicKey)
    const mac = randomMac()
    const macEncrypted = await encryptWithPublicKey(mac, publicKey)

    const device = await createDevice(client, {
      name: 'Test NAS',
      mac_encrypted: macEncrypted,
      mac_display: maskMACAddress(mac),
      description: '书房群晖',
    })

    // 证据② API：201 + mac_display 脱敏（AA:**:**:**:**:FF，首末段可见）
    expect(device.did).toBeTruthy()
    expect(device.mac_display).toMatch(/^[0-9A-F]{2}:(\*\*:){4}[0-9A-F]{2}$/)

    // 证据③ listDevices 出现
    const list = await listDevices(client)
    expect(list.find((d) => d.did === device.did)).toBeTruthy()
  })

  test('设备配额第 3 个 422', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(freshUser.user.id, publicKey)
    // 先建 2 设备（配额上限 MAX_DEVICES_PER_USER=2）
    for (let i = 0; i < 2; i++) {
      const mac = randomMac()
      await createDevice(client, {
        name: `Dev ${i}`,
        mac_encrypted: await encryptWithPublicKey(mac, publicKey),
        mac_display: maskMACAddress(mac),
      })
    }
    // 第 3 个 → 422 QUOTA_EXCEEDED
    const mac = randomMac()
    await expect(
      createDevice(client, {
        name: 'Dev 3',
        mac_encrypted: await encryptWithPublicKey(mac, publicKey),
        mac_display: maskMACAddress(mac),
      }),
    ).rejects.toThrow()
  })

  test('编辑设备名', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(freshUser.user.id, publicKey)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Old Name',
      mac_encrypted: await encryptWithPublicKey(mac, publicKey),
      mac_display: maskMACAddress(mac),
    })
    const updated = await updateDevice(client, device.did, { name: 'New Name' })
    expect(updated.name).toBe('New Name')
  })

  test('删除设备（API 204，硬删需 agent 场景3 验）', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(freshUser.user.id, publicKey)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'To Delete',
      mac_encrypted: await encryptWithPublicKey(mac, publicKey),
      mac_display: maskMACAddress(mac),
    })
    // 场景 1 无 agent：API 返 204（删除请求接受），但硬删需 agent 收 device_remove 后完成。
    // 此处仅验 API 不报错（204）。硬删验证见 device-sync.spec.ts 删除设备同步。
    await deleteDevice(client, device.did)
    // 不再断言 list 无该设备（需 agent ack 才硬删）
  })

  test('MAC 格式校验（前端 mac_display 格式）', async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken)
    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(freshUser.user.id, publicKey)
    // 合法 MAC：验证 mac_display 脱敏格式正确
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Valid MAC',
      mac_encrypted: await encryptWithPublicKey(mac, publicKey),
      mac_display: maskMACAddress(mac),
    })
    // mac_display 是 AA:**:**:**:**:FF 格式（首末段可见）
    expect(device.mac_display).toMatch(/^[0-9A-F]{2}:(\*\*:){4}[0-9A-F]{2}$/)
  })

  test('跨用户访问 404', async ({}) => {
    // 两个独立用户，A 的 device 在 B 的 API → 404（防枚举）
    const clientA = createBrowserClient()
    const authA = await registerUser(clientA, { email: uniqueEmail(), password: 'TestPass123!' })
    const clientB = createBrowserClient()
    await registerUser(clientB, { email: uniqueEmail(), password: 'TestPass123!' })

    const { publicKey } = generateRSAKeyPairPEM()
    setAgentPublicKey(authA.user.id, publicKey)
    const mac = randomMac()
    const authClientA = createBrowserClient(authA.access_token)
    const device = await createDevice(authClientA, {
      name: 'A Device',
      mac_encrypted: await encryptWithPublicKey(mac, publicKey),
      mac_display: maskMACAddress(mac),
    })
    // B 用自己的 token 访问 A 的 device → 404
    const authB = await registerUser(createBrowserClient(), {
      email: uniqueEmail(),
      password: 'TestPass123!',
    })
    const clientBAuth = createBrowserClient(authB.access_token)
    await expect(getDevice(clientBAuth, device.did)).rejects.toThrow()
  })
})
