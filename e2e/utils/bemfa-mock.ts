/**
 * Bemfa mock 调试端点访问（device-sync-v3 e2e）。
 *
 * bemfa-mock 容器无发布端口，仅 e2e-net 内可达。通过 docker exec 在容器内 curl 访问调试端点。
 * 调试端点（server.js）：
 *   GET  /__debug/topics   → { topics: [...], detail: {...} }
 *   POST /__debug/reset    → 清空 topic 表
 *   POST /__debug/delete   → {topic} 模拟云端删 topic（触发漂移 deleted）
 *   POST /__debug/rename   → {topic, name} 模拟云端改名（触发漂移 renamed）
 */
import { execSync } from 'node:child_process'

const CONTAINER = 'e2e-bemfa-mock-1'
const BASE = 'http://localhost:8080'

function execInMock(args: string): string {
  return execSync(`docker exec ${CONTAINER} wget -qO- ${args}`, {
    timeout: 10_000,
  }).toString()
}

function execInMockPost(path: string, body: Record<string, unknown>): string {
  const json = JSON.stringify(body).replace(/'/g, "'\\''")
  return execSync(
    `docker exec ${CONTAINER} sh -c "wget -qO- --header='Content-Type: application/json' --post-data='${json}' ${BASE}${path}"`,
    { timeout: 10_000 },
  ).toString()
}

/** 获取 mock 当前记录的所有 topic（断言 agent createTopic 成功）。 */
export function getMockTopics(): string[] {
  const raw = execInMock(`${BASE}/__debug/topics`)
  try {
    const parsed = JSON.parse(raw)
    return parsed.topics ?? []
  } catch {
    return []
  }
}

/** 清空 mock topic 表（测试隔离）。 */
export function resetMock(): void {
  execInMockPost('/__debug/reset', {})
}

/** 模拟用户在云端删 topic（触发 agent 漂移 deleted，§9 情形 7）。 */
export function mockCloudDeleteTopic(topic: string): void {
  execInMockPost('/__debug/delete', { topic })
}

/** 模拟用户在云端改 topic 昵称（触发 agent 漂移 renamed，§9 情形 8）。 */
export function mockCloudRenameTopic(topic: string, name: string): void {
  execInMockPost('/__debug/rename', { topic, name })
}

/** 构造 device 的 topic 名：ww{k4}{did}006。需与 agent bemfa::device_topic 一致。 */
export function deviceTopic(aidSimple: string, didSimple: string): string {
  // k4 = base36(sha256(aid)[..8] big-endian)[..4] —— agent 端用 sha2 计算。
  // e2e 测试里我们不知道 k4，但可通过 getMockTopics 找到含 didSimple 的 topic。
  // 故此函数仅作占位；实际测试用 findMockTopicForDevice。
  return `ww???${didSimple}006`
}

/** 在 mock topic 表中找含指定 didSimple 的 topic（k4 未知，按 did 后缀匹配）。 */
export function findMockTopicForDevice(didSimple: string): string | undefined {
  const topics = getMockTopics()
  return topics.find((t) => t.includes(didSimple) && t.endsWith('006'))
}
