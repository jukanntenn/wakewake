'use client'

// PoW 求解封装（ui-ux-risk-control §7.1/§7.3）。
// 优先 Web Worker（不阻塞 UI），失败降级主线程（setTimeout 间隙更新进度）。
// 被 register 和 forgot-password 复用（各自调 solvePow）。

export interface PowResult {
  challenge: string
  nonce: string
}

/**
 * 求解 PoW：先 GET /pow/challenge 取 challenge，再求 nonce。
 * @param onProgress 进度回调（可选，用于更新进度条）
 */
export async function solvePow(
  apiGetChallenge: () => Promise<{ id: string; challenge: string; difficulty: number }>,
  onProgress?: () => void,
): Promise<PowResult> {
  const ch = await apiGetChallenge()
  const nonce = await solveChallenge(ch.challenge, ch.difficulty, onProgress)
  return { challenge: ch.id, nonce }
}

/** 求解单个 challenge。优先 Worker，降级主线程。 */
export function solveChallenge(
  challenge: string,
  difficulty: number,
  onProgress?: () => void,
): Promise<string> {
  // 尝试 Web Worker
  if (typeof Worker !== 'undefined') {
    try {
      return solveInWorker(challenge, difficulty, onProgress)
    } catch {
      // Worker 创建失败 → 降级主线程
    }
  }
  return solveInMain(challenge, difficulty, onProgress)
}

function solveInWorker(
  challenge: string,
  difficulty: number,
  onProgress?: () => void,
): Promise<string> {
  return new Promise((resolve, reject) => {
    // 用 Blob URL 内联 Worker 代码（避免独立 .ts worker 文件被 Next.js 类型检查冲突）。
    const workerCode = `
      self.onmessage = async (e) => {
        const { challenge, difficulty } = e.data;
        const prefix = '0'.repeat(difficulty);
        const encoder = new TextEncoder();
        let nonce = 0;
        const batchSize = 5000;
        while (true) {
          const data = encoder.encode(challenge + nonce.toString());
          const hashBuf = await crypto.subtle.digest('SHA-256', data);
          const bytes = new Uint8Array(hashBuf);
          let hex = '';
          for (const b of bytes) hex += b.toString(16).padStart(2, '0');
          if (hex.startsWith(prefix)) {
            self.postMessage({ type: 'nonce', nonce: nonce.toString() });
            return;
          }
          nonce++;
          if (nonce % batchSize === 0) {
            self.postMessage({ type: 'progress', count: nonce });
          }
        }
      };
    `
    const blob = new Blob([workerCode], { type: 'application/javascript' })
    const worker = new Worker(URL.createObjectURL(blob))
    worker.onmessage = (e: MessageEvent) => {
      const msg = e.data
      if (msg.type === 'progress') {
        onProgress?.()
      } else if (msg.type === 'nonce') {
        worker.terminate()
        resolve(msg.nonce)
      }
    }
    worker.onerror = () => {
      worker.terminate()
      // Worker 运行时错误 → 降级主线程
      solveInMain(challenge, difficulty, onProgress).then(resolve, reject)
    }
    worker.postMessage({ challenge, difficulty })
  })
}

/** 主线程求解（降级）。用 setTimeout(0) 间隙让出，避免完全冻结 UI。 */
async function solveInMain(
  challenge: string,
  difficulty: number,
  onProgress?: () => void,
): Promise<string> {
  const prefix = '0'.repeat(difficulty)
  const encoder = new TextEncoder()
  let nonce = 0
  const batchSize = 5000

  while (true) {
    const data = encoder.encode(challenge + nonce.toString())
    // Node 和浏览器都有 crypto.subtle
    const hashBuf = await crypto.subtle.digest('SHA-256', data)
    const hex = bufferToHex(hashBuf)
    if (hex.startsWith(prefix)) {
      return nonce.toString()
    }
    nonce++
    if (nonce % batchSize === 0) {
      onProgress?.()
      // 让出主线程（§7.3：setTimeout(0) 间隙更新进度条 DOM）
      await new Promise((r) => setTimeout(r, 0))
    }
  }
}

function bufferToHex(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf)
  let hex = ''
  for (const b of bytes) {
    hex += b.toString(16).padStart(2, '0')
  }
  return hex
}
