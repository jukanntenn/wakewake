/**
 * 场景 1：用户↔server（无 agent）—— 集成（e2e.md §7.2 integrations.spec.ts P1）。
 *
 * 5 用例：bemfa schema/创建/secret 加密/禁用启用/删除。
 */
import { test, expect } from "../fixtures";
import {
  createBrowserClient,
  getIntegrationSchema,
  createIntegration,
  disableIntegration,
  enableIntegration,
  deleteIntegration,
  listIntegrations,
} from "../utils/api";
import { generateRSAKeyPairPEM, encryptWithPublicKey } from "../utils/crypto";
import { setAgentPublicKey, getIntegrationConfigField } from "../utils/db";

test.describe("集成（场景 1，无 agent）", () => {
  test("获取 bemfa schema", async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken);
    const schema = await getIntegrationSchema(client, "bemfa");
    // JSON Schema 形态（含 properties.uid secret:true）
    expect(schema).toBeTruthy();
    expect(JSON.stringify(schema)).toContain("uid");
  });

  test("创建 bemfa 集成", async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken);
    const { publicKey } = generateRSAKeyPairPEM();
    setAgentPublicKey(freshUser.user.id, publicKey);
    const uidEncrypted = await encryptWithPublicKey(
      "test-bemfa-uid",
      publicKey,
    );
    const integration = await createIntegration(client, "bemfa", {
      uid: uidEncrypted,
    });
    // API 201（ky 不抛即成功）+ uid 脱敏（***）
    expect(integration.provider).toBe("bemfa");
    expect(integration.enabled).toBe(true);
    // config.uid 返回 *** 脱敏
    expect(JSON.stringify(integration.config)).toMatch(/\*\*\*/);
  });

  test("secret 字段加密（DB 直查）", async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken);
    const { publicKey } = generateRSAKeyPairPEM();
    setAgentPublicKey(freshUser.user.id, publicKey);
    const uidPlain = "secret-uid-12345";
    const uidEncrypted = await encryptWithPublicKey(uidPlain, publicKey);
    await createIntegration(client, "bemfa", {
      uid: uidEncrypted,
    });
    // DB 直查：uid 是 RSA 密文（base64），非明文
    const dbUid = getIntegrationConfigField("bemfa", "uid");
    expect(dbUid).not.toBe(uidPlain);
    expect(dbUid.length).toBeGreaterThan(50); // RSA 密文较长
  });

  test("禁用/启用集成", async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken);
    const { publicKey } = generateRSAKeyPairPEM();
    setAgentPublicKey(freshUser.user.id, publicKey);
    await createIntegration(client, "bemfa", {
      uid: await encryptWithPublicKey("uid-test", publicKey),
    });
    const disabled = await disableIntegration(client, "bemfa");
    expect(disabled.enabled).toBe(false);
    const enabled = await enableIntegration(client, "bemfa");
    expect(enabled.enabled).toBe(true);
  });

  test("删除集成（API 204，硬删需 agent 场景3 验）", async ({ freshUser }) => {
    const client = createBrowserClient(freshUser.accessToken);
    const { publicKey } = generateRSAKeyPairPEM();
    setAgentPublicKey(freshUser.user.id, publicKey);
    await createIntegration(client, "bemfa", {
      uid: await encryptWithPublicKey("uid-del", publicKey),
    });
    // 场景 1 无 agent：API 接受删除，硬删需 agent 收 integration_remove 后完成。
    await deleteIntegration(client, "bemfa");
    // 不再断言 list 无该 integration（需 agent ack 才硬删）
  });
});
