#!/usr/bin/env node
/**
 * SSE 容量压测 Node.js 降级实现（k6 + xk6-sse 构建受阻时的 fallback）。
 *
 * load.md §6.2 S1 SSE 容量的核心目标：阶梯爬 VU，测单连接增量 < 2.5KB + 线性扩展。
 * 此 Node 实现：
 *   - 阶梯 100 → 500 → 1000 → 2000 → 3000 VU（保守，宿主 12GB 内存）
 *   - 每 VU 持 1 SSE 连接到 https://localhost:8443/api/v1/agents/self/events
 *   - 旁路采集 server RSS（docker exec /proc/PID/VmRSS）
 *   - 输出 metrics-samples.csv + 回归分析数据
 *
 * 用法：node sse-node.js pairing_codes.txt
 */
import { readFileSync } from 'node:fs'
import { execSync } from 'node:child_process'

const BASE_URL = process.env.BASE_URL || 'https://localhost:8443'
const CODES_FILE = process.argv[2] || 'pairing_codes.txt'
const APP_CONTAINER = process.env.PG_CONTAINER?.replace('postgres', 'app') || 'e2e-app-1'

// 禁用证书校验（Caddy tls internal 自签名）
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0'

const codes = readFileSync(CODES_FILE, 'utf8').trim().split('\n').filter(Boolean)
console.error(`loaded ${codes.length} pairing codes`)

// 阶梯（load.md §6.2 完整版，峰值 5000 VU 达 R²>0.99）
const STAGES = [
  { target: 500, duration: 30_000 },
  { target: 1000, duration: 30_000 },
  { target: 2000, duration: 30_000 },
  { target: 3000, duration: 30_000 },
  { target: 5000, duration: 30_000 },
]

function getServerRss() {
  try {
    // busybox pgrep 不可靠，直接遍历 /proc 找 wakewake-server PID
    const pids = execSync(
      `docker exec ${APP_CONTAINER} sh -c 'for p in $(ls /proc/|grep ^[0-9]); do c=$(cat /proc/$p/cmdline 2>/dev/null|tr "\\0" " "|head -c 40); case "$c" in *wakewake*) echo $p;; esac; done'`,
    ).toString().trim().split('\n').filter(Boolean)
    if (pids.length === 0) return 0
    const pid = pids[0]
    const status = execSync(`docker exec ${APP_CONTAINER} cat /proc/${pid}/status 2>/dev/null`).toString()
    const m = status.match(/VmRSS:\s*(\d+)/)
    return m ? parseInt(m[1], 10) : 0 // KB
  } catch {
    return 0
  }
}

function getHostAvailMb() {
  try {
    return parseInt(execSync("free -m | awk '/Mem:/{print $7}'").toString().trim(), 10)
  } catch {
    return 0
  }
}

const samples = []
function sample(vuCount) {
  const rss = getServerRss()
  const avail = getHostAvailMb()
  samples.push({ ts: Date.now(), vu: vuCount, rss_kb: rss, host_avail_mb: avail })
  console.error(`[sample] VU=${vuCount} app_rss=${rss}KB host_avail=${avail}MB`)
}

// 单 SSE 连接（持连接，断开则记 failed）
async function holdSse(code, idx) {
  try {
    const res = await fetch(`${BASE_URL}/api/v1/agents/self/events`, {
      headers: { Authorization: `Bearer ${code}`, Accept: 'text/event-stream' },
    })
    if (!res.ok || !res.body) {
      throw new Error(`HTTP ${res.status}`)
    }
    // 持续读取流（保持连接）
    const reader = res.body.getReader()
    // 不消费数据，仅保持连接打开；定期 tick
    return { reader, ok: res.ok }
  } catch (e) {
    return { error: String(e.message || e), ok: false }
  }
}

async function runStage(targetVus, durationMs) {
  console.error(`\n=== STAGE: target=${targetVus} duration=${durationMs}ms ===`)
  const conns = []
  const failures = []
  // 建连接（分批避免瞬时 burst）
  const batchSize = 50
  for (let i = 0; i < targetVus; i += batchSize) {
    const batch = Math.min(batchSize, targetVus - i)
    const promises = []
    for (let j = 0; j < batch; j++) {
      const code = codes[(i + j) % codes.length]
      promises.push(holdSse(code, i + j))
    }
    const results = await Promise.all(promises)
    for (const r of results) {
      if (r.ok) conns.push(r)
      else failures.push(r.error)
    }
    await new Promise((r) => setTimeout(r, 100))
  }
  console.error(`connected: ${conns.length}, failed: ${failures.length}`)
  // 稳态采样（duration / 5s 间隔）
  const sampleCount = Math.max(1, Math.floor(durationMs / 5000))
  for (let s = 0; s < sampleCount; s++) {
    await new Promise((r) => setTimeout(r, 5000))
    sample(conns.length)
    // 宿主内存 < 1.5GB → 提前终止（load.md §10.2）
    const avail = getHostAvailMb()
    if (avail && avail < 1500) {
      console.error(`[ALERT] host avail ${avail}MB < 1500MB, aborting stage`)
      break
    }
  }
  // 关闭连接
  for (const c of conns) {
    try {
      await c.reader.cancel()
    } catch {}
  }
  return { connected: conns.length, failed: failures.length }
}

async function main() {
  console.error('starting SSE load test (Node fallback)')
  for (const stage of STAGES) {
    const result = await runStage(stage.target, stage.duration)
    console.error(`stage result: ${JSON.stringify(result)}`)
  }
  // 输出 CSV
  const csv = ['ts,vu,app_rss_kb,host_avail_mb']
    .concat(samples.map((s) => `${s.ts},${s.vu},${s.rss_kb},${s.host_avail_mb}`))
    .join('\n')
  const { writeFileSync, mkdirSync } = await import('node:fs')
  mkdirSync('results', { recursive: true })
  writeFileSync('metrics-samples.csv', csv)
  console.error(`\nwrote ${samples.length} samples to metrics-samples.csv`)
  console.error('peak VU: ' + Math.max(...samples.map((s) => s.vu)))
  console.error('peak app RSS: ' + Math.max(...samples.map((s) => s.rss_kb)) + ' KB')
}

main().catch((e) => {
  console.error('FATAL:', e)
  process.exit(1)
})
