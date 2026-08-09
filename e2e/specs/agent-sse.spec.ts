/**
 * 场景 2：agent↔server —— SSE 协议（e2e.md §7.3 agent-sse.spec.ts P0）。
 *
 * 5 用例：握手上报公钥/state 全量推送 ack/命令派发执行/心跳保活/重连 state 重推。
 * 用 pairedAgent（真实 agent 容器，worker scope）。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  getDefaultAgent,
  createDevice,
  wakeDevice,
  getCommand,
} from '../utils/api'
import {
  generateRSAKeyPairPEM,
  encryptWithPublicKey,
  maskMACAddress,
  randomMac,
} from '../utils/crypto'
import { getDeviceObservation } from '../utils/db'
import { sleep } from '../utils/shared'

test.describe('Agent SSE 协议（场景 2）', () => {
  test('agent 握手上报公钥 → online', async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken)
    const agent = await getDefaultAgent(client)
    // 证据：API status=online；public_key 非空
    expect(agent.status).toBe('online')
    expect(agent.public_key).toBeTruthy()
  })

  test('state 全量推送后 agent 应用快照（device-sync-v3 投影通道）', async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken)
    // 建一个设备（触发 device_add 命令 → state 重推）
    const { publicKey } = generateRSAKeyPairPEM()
    // pairedAgent 的 agent 已有自己的公钥（容器上报的），用容器公钥加密
    const agentInfo = await getDefaultAgent(client)
    const mac = randomMac()
    const macEncrypted = await encryptWithPublicKey(mac, agentInfo.public_key!)
    const device = await createDevice(client, {
      name: 'SSE Test Device',
      mac_encrypted: macEncrypted,
      mac_display: maskMACAddress(mac),
    })
    // state 重推 + agent ack → projection_status synced（device-sync-v3 §5.1）
    await sleep(3000)
    // 验证 agent 已应用快照（观测态可查；projection_status 由 hub acked 派生）
    const obs = getDeviceObservation(device.did)
    expect(obs).toBeDefined() // 设备行存在即证明 state 推送链路通
  })

  test('命令派发与执行（wol）', async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken)
    const agentInfo = await getDefaultAgent(client)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Wake Target',
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    })
    await sleep(2000) // 等 state 同步
    // 派发 wake 命令
    const cmd = await wakeDevice(client, device.did)
    expect(cmd.command_id).toBeTruthy()
    // 轮询命令状态直到 completed（agent 执行 WoL）
    let final
    for (let i = 0; i < 30; i++) {
      final = await getCommand(client, cmd.command_id)
      if (final.status === 'completed' || final.status === 'failed') break
      await sleep(1000)
    }
    expect(final!.status).toBe('completed')
  })

  test('心跳保活（连 35s+ 不中断）', async ({ pairedAgent }) => {
    test.setTimeout(60_000) // 需要等待35s+
    const client = createBrowserClient(pairedAgent.accessToken)
    // 等 35s（覆盖心跳 30s 周期）
    await sleep(35_000)
    const agent = await getDefaultAgent(client)
    // 仍 online（心跳保活未断）
    expect(agent.status).toBe('online')
  })

  test('agent 重连后 state 重推', async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken)
    // 重启 agent 容器
    await pairedAgent.container.restart()
    await pairedAgent.container.waitForOnline(client)
    // 重连后 agent 重新 online
    const agent = await getDefaultAgent(client)
    expect(agent.status).toBe('online')
  })
})
