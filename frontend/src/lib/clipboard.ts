/**
 * 剪贴板工具：封装 copy-to-clipboard 库。
 *
 * 该库优先用 navigator.clipboard（secure context），不可用时降级到 execCommand
 * （经典隐藏 textarea 方案）——覆盖 HTTP 非 localhost 部署下的 secure-only API 限制。
 *
 * 统一封装一层便于：(a) 全局调用入口一致；(b) 将来加埋点只改一处；(c) 测试 mock。
 *
 * v4 起 copy-to-clipboard 改为 async（Clipboard API 本身是 Promise-based，execCommand
 * 降级在库内部处理）。调用方需 await 结果以区分成功/失败路径。
 *
 * @returns true=复制成功（任一路径），false=两条路径都失败
 */
import copy from 'copy-to-clipboard'

export async function copyText(text: string): Promise<boolean> {
  return copy(text)
}
