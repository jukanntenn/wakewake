/**
 * E2E 共享工具（e2e.md §3.2 utils/shared.ts）。
 * uniqueId / sleep / 随机数据生成。
 */

/** 唯一 ID（时间戳 + 随机后缀，避免并行测试冲突）。 */
export function uniqueId(prefix = ''): string {
  const ts = Date.now().toString(36)
  const rand = Math.random().toString(36).slice(2, 8)
  return `${prefix}${ts}${rand}`
}

/** 生成唯一测试邮箱（e2e.md §4.4 freshUser 用）。 */
export function uniqueEmail(domain = 'e2e.wakewake.local'): string {
  return `user-${uniqueId()}@${domain}`
}

/** sleep（毫秒）。 */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/** 轮询直到条件满足或超时（agent online 等待用）。 */
export async function waitFor<T>(
  fn: () => Promise<T>,
  predicate: (value: T) => boolean,
  opts: { timeoutMs?: number; intervalMs?: number } = {},
): Promise<T> {
  const { timeoutMs = 30_000, intervalMs = 1000 } = opts
  const deadline = Date.now() + timeoutMs
  let last: T
  do {
    last = await fn()
    if (predicate(last)) return last
    await sleep(intervalMs)
  } while (Date.now() < deadline)
  throw new Error(`waitFor timed out after ${timeoutMs}ms`)
}
