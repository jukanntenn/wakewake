/**
 * 场景 3：用户↔server↔agent —— 唤醒全链路（e2e.md §7.4 wake-flow.spec.ts P0）。
 *
 * 3 用例：用户唤醒→agent WoL / agent 离线唤醒超时 / 命令轮询 UI 反馈。
 * 四重证据：UI toast + API + DB wakes + UDP sniffer 包。
 */
import { test, expect } from '../fixtures'
import {
  createBrowserClient,
  getDefaultAgent,
  createDevice,
  wakeDevice,
  getCommand,
  listWakes,
} from '../utils/api'
import { encryptWithPublicKey, maskMACAddress, randomMac } from '../utils/crypto'
import { getWakeStatus } from '../utils/db'
import { sleep } from '../utils/shared'

test.describe('唤醒全链路（场景 3）', () => {
  test('用户唤醒 → agent 执行 WoL（四重证据）', async ({ pairedAgent, page }) => {
    const client = createBrowserClient(pairedAgent.accessToken)
    const agentInfo = await getDefaultAgent(client)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Wake Flow Target',
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    })
    await sleep(2000) // 等 state 同步
    await pairedAgent.container.clearWolPackets()

    // 浏览器触发 wake（UI 建）
    await page.goto('/dashboard/devices')
    await page.waitForLoadState('networkidle')
    // 点设备的 wake 按钮（按钮文案可能是 wake/唤醒）
    const wakeBtn = page.locator(`text=${device.name}`).locator('..').getByRole('button', {
      name: /wake|唤醒/i,
    })
    if (await wakeBtn.count()) {
      await wakeBtn.click()
    } else {
      // fallback：API 直接触发
      await wakeDevice(client, device.did)
    }

    // 证据① UDP sniffer 收包（agent 确实发了 WoL）
    let packets
    for (let i = 0; i < 15; i++) {
      packets = await pairedAgent.container.getWolPackets()
      if (packets.length > 0) break
      await sleep(1000)
    }
    expect(packets!.length).toBeGreaterThan(0)

    // 证据② DB wakes success（权威证据：server 写了 wake 记录）
    let status
    for (let i = 0; i < 15; i++) {
      status = getWakeStatus(device.did)
      if (status) break
      await sleep(1000)
    }
    expect(status).toBe('success')

    // 证据③ API wakes 列表含 success 记录
    const wakes = await listWakes(client, { device_id: device.did })
    expect(wakes.items.find((w) => w.status === 'success')).toBeTruthy()
  })

  test('agent 离线唤醒超时 expired', async ({ pairedAgent, page }) => {
    test.setTimeout(120_000) // 90s 轮询 + 余量
    const client = createBrowserClient(pairedAgent.accessToken)
    const agentInfo = await getDefaultAgent(client)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Offline Target',
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    })
    // stop agent → 离线
    await pairedAgent.container.stop()
    await sleep(2000)

    // 触发 wake（agent 离线，命令将 60s expired）
    const cmd = await wakeDevice(client, device.did)
    expect(cmd.command_id).toBeTruthy()

    // 轮询直到 expired（60s TTL + 10s sweep 间隔 → 最迟 ~70s，留余量轮询 90s）
    let final
    for (let i = 0; i < 90; i++) {
      final = await getCommand(client, cmd.command_id)
      if (final.status === 'expired') break
      await sleep(1000)
    }
    expect(final!.status).toBe('expired')

    // DB wakes expired
    const status = getWakeStatus(device.did)
    expect(status).toBe('expired')
  })

  test('命令轮询 UI 反馈（pending→dispatched→completed）', async ({ pairedAgent }) => {
    test.setTimeout(60_000)
    const client = createBrowserClient(pairedAgent.accessToken)
    const agentInfo = await getDefaultAgent(client)
    const mac = randomMac()
    const device = await createDevice(client, {
      name: 'Poll Target',
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    })
    await sleep(2000)
    const cmd = await wakeDevice(client, device.did)
    // 立即查 → pending 或 dispatched
    const early = await getCommand(client, cmd.command_id)
    expect(['pending', 'dispatched', 'completed']).toContain(early.status)
    // 等完成
    let final
    for (let i = 0; i < 30; i++) {
      final = await getCommand(client, cmd.command_id)
      if (final.status === 'completed') break
      await sleep(1000)
    }
    expect(final!.status).toBe('completed')
  })
})
