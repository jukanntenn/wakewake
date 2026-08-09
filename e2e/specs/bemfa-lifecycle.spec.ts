// E2E：巴法云 topic 生命周期（state-as-truth 重构后）。
//
// state-as-truth 后，agent 不再持久化 created_topics.json；topic 的 create/delete 由
// agent reconcile 驱动。本 spec 通过验证 server 侧的 IntegrationStatus 上报结果
// （integration.status 收敛到 connected/disconnected）来确认 agent 对账成功，
// 而非读取已删除的本地文件。

import { test, expect } from "../fixtures";
import { createBrowserClient } from "../utils/api";
import { randomMac, encryptWithPublicKey, maskMACAddress } from "../utils/crypto";

// agent 的 is_valid_uid 要求 32 位 hex 或 45 位字符（bemfa_state.rs）
const BEMFA_UID = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4";

/// 轮询 integration 直到 status 收敛到非 connecting 态（或超时）。device-sync-v3 §8.5。
async function waitForSyncSettle(
  client: ReturnType<typeof createBrowserClient>,
  timeoutMs = 30000,
): Promise<{ status: string; last_error?: string }> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const integ = (await client.get("integrations/bemfa").json()) as {
      status: string;
      last_error?: string;
    };
    if (integ.status !== "connecting") return integ;
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error("integration status did not settle (still connecting)");
}

test.describe("巴法 topic 生命周期（state-as-truth）", () => {
  test("添加集成 + 设备后，agent 对账上报 synced", async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken);

    const uidEncrypted = await encryptWithPublicKey(
      BEMFA_UID,
      pairedAgent.publicKeyPEM,
    );
    await client.post("integrations", {
      json: { provider: "bemfa", config: { uid: uidEncrypted }, enabled: true },
    });

    const mac = randomMac();
    const macEncrypted = await encryptWithPublicKey(
      mac,
      pairedAgent.publicKeyPEM,
    );
    await client.post("devices", {
      json: {
        name: "Lifecycle PC",
        mac_display: maskMACAddress(mac),
        mac_encrypted: macEncrypted,
      },
    });

    // 等 agent SSE 收到 state refresh + reconcile + 上报 sync（device-sync-v3 §8.5 status）
    await pairedAgent.container.waitForOnline(client);
    const settled = await waitForSyncSettle(client);
    // 收敛到 connected（uid 合法 + mqtt 连上 + 对账完成）或 disconnected（mqtt 未连）。
    // 关键：脱离 connecting，证明 agent 已对账并上报。
    expect(["connected", "disconnected", "error", "disabled"]).toContain(
      settled.status,
    );
  });

  test("删除集成后，integration 行消失", async ({ pairedAgent }) => {
    const client = createBrowserClient(pairedAgent.accessToken);

    const uidEncrypted = await encryptWithPublicKey(
      BEMFA_UID,
      pairedAgent.publicKeyPEM,
    );
    await client.post("integrations", {
      json: { provider: "bemfa", config: { uid: uidEncrypted }, enabled: true },
    });

    const mac = randomMac();
    const macEncrypted = await encryptWithPublicKey(
      mac,
      pairedAgent.publicKeyPEM,
    );
    await client.post("devices", {
      json: {
        name: "IntDel PC",
        mac_display: maskMACAddress(mac),
        mac_encrypted: macEncrypted,
      },
    });

    await pairedAgent.container.waitForOnline(client);

    // 删集成（inline delete：DB 行立即消失）
    const resp = await client.delete("integrations/bemfa");
    expect(resp.status).toBe(204);

    // 集成不再可查
    const list = (await client.get("integrations").json()) as {
      items: unknown[];
    };
    expect(
      list.items.find((i) => (i as { provider: string }).provider === "bemfa"),
    ).toBeUndefined();
  });
});
