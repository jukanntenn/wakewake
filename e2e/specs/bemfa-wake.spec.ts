/**
 * 场景 2：agent↔server —— bemfa 语音唤醒（e2e.md §7.3 bemfa-wake.spec.ts P2）。
 *
 * 2 用例：MQTT on 触发 WoL / MQTT off 不触发。
 * 用 mosquitto mock 容器（profiles: [bemfa]）模拟巴法 MQTT。
 *
 * agent 的 bemfa 模块（WAKEWAKE_BEMFA__BROKER=mosquitto）订阅 device topic。
 * 测试通过 mosquitto CLI publish "on" 模拟语音触发。
 */
import { test, expect } from "../fixtures";
import {
  createBrowserClient,
  createDevice,
  getDefaultAgent,
  createIntegration,
  listWakes,
} from "../utils/api";
import {
  encryptWithPublicKey,
  maskMACAddress,
  randomMac,
} from "../utils/crypto";
import { execSync } from "node:child_process";
import { sleep } from "../utils/shared";
import { resetMock, findMockTopicForDevice } from "../utils/bemfa-mock";

/** mosquitto publish（容器内或临时 docker run fallback）。 */
function mosquittoPub(topic: string, message: string) {
  try {
    execSync(
      `docker exec e2e-mosquitto-1 mosquitto_pub -h 127.0.0.1 -t ${topic} -m "${message}"`,
      { stdio: "inherit" },
    );
  } catch {
    execSync(
      `docker run --rm --network e2e_e2e-net eclipse-mosquitto:2 mosquitto_pub -h mosquitto -t ${topic} -m "${message}"`,
      { stdio: "inherit" },
    );
  }
}

test.describe("Bemfa 语音唤醒（场景 2，需 mosquitto）", () => {
  test("MQTT on 触发 WoL（agent MQTT loop 完整流程）", async ({
    pairedAgent,
  }) => {
    test.setTimeout(90_000);
    const client = createBrowserClient(pairedAgent.accessToken);
    const agentInfo = await getDefaultAgent(client);
    const mac = randomMac();
    // topic 前缀固定为 ww（代码常量，不再用户可配）
    // uid 必须满足 agent is_valid_uid（32 hex 或 45 字符），否则 MQTT loop 不启动
    const uidEncrypted = await encryptWithPublicKey(
      "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4",
      agentInfo.public_key!,
    );
    // 创建 bemfa 集成 → agent 收 state → 启动 bemfa MQTT loop
    resetMock();
    await createIntegration(client, "bemfa", {
      uid: await encryptWithPublicKey("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4", agentInfo.public_key!),
    });
    const device = await createDevice(client, {
      name: "Bemfa Target",
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    });
    // 等 agent 处理 + bemfa 解密 uid + 订阅 mosquitto + createTopic（mock 记录 topic）
    const didSimple = device.did.replace(/-/g, "");
    let topic: string | undefined;
    for (let i = 0; i < 30; i++) {
      topic = findMockTopicForDevice(didSimple);
      if (topic) break;
      await sleep(2000);
    }
    expect(topic, "device topic should appear in mock after reconcile").toBeTruthy();
    await sleep(3000); // 额外等 MQTT 订阅生效
    await pairedAgent.container.clearWolPackets();

    // publish "on" 到设备 topic（device-sync-v3 含 k4：ww{k4}{did}006，从 mock 取实际名）
    mosquittoPub(topic!, "on");
    await sleep(3000);

    // 证据① WoL sniffer 收包（agent 收到 MQTT on → 解密 MAC → 发 WoL）
    let packets;
    for (let i = 0; i < 10; i++) {
      packets = await pairedAgent.container.getWolPackets();
      if (packets.length > 0) break;
      await sleep(1000);
    }
    // 证据② DB wakes 含 bemfa_wake 记录
    let bemfaWake;
    for (let i = 0; i < 10; i++) {
      const wakes = await listWakes(client, { device_id: device.did });
      bemfaWake = wakes.items.find((w) => w.type === "bemfa_wake");
      if (bemfaWake) break;
      await sleep(1000);
    }
    expect(bemfaWake).toBeTruthy();
    expect(bemfaWake!.status).toBe("success");
  });

  test("MQTT off 不触发 WoL", async ({ pairedAgent }) => {
    test.setTimeout(60_000);
    const client = createBrowserClient(pairedAgent.accessToken);
    const agentInfo = await getDefaultAgent(client);
    const mac = randomMac();
    resetMock();
    // topic 前缀固定为 ww
    await createIntegration(client, "bemfa", {
      uid: await encryptWithPublicKey("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4", agentInfo.public_key!),
    });
    const device = await createDevice(client, {
      name: "Bemfa Off Target",
      mac_encrypted: await encryptWithPublicKey(mac, agentInfo.public_key!),
      mac_display: maskMACAddress(mac),
    });
    // 等 agent 处理 + 订阅 mosquitto + createTopic
    const didSimple = device.did.replace(/-/g, "");
    let topic: string | undefined;
    for (let i = 0; i < 30; i++) {
      topic = findMockTopicForDevice(didSimple);
      if (topic) break;
      await sleep(2000);
    }
    expect(topic).toBeTruthy();
    await sleep(3000);
    await pairedAgent.container.clearWolPackets();

    // publish "off"（不应触发 WoL）
    mosquittoPub(topic!, "off");
    await sleep(3000);

    // 无 WoL 包
    const packets = await pairedAgent.container.getWolPackets();
    expect(packets.length).toBe(0);
    // 无 bemfa_wake 记录
    const wakes = await listWakes(client, { device_id: device.did });
    expect(wakes.items.find((w) => w.type === "bemfa_wake")).toBeUndefined();
  });
});
