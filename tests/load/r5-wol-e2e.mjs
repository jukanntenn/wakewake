// R5 真实 wake 端到端（specs/testing/load.md §R5）—— 核心业务 SLO 实测。
//
// 真实链路：真实用户 → 真实 agent 容器（生产 agent 镜像）→ RSA-OAEP 解密 →
// WoL 发包 → 回报 /agents/self/wakes。测 wake 发起 → wakes 落行的完整往返。
//
// 三阶段（由 run-prod-sim.sh 编排，psql 直改 email_verified 与 e2e §4.5 同先例）：
//   node r5-wol-e2e.mjs prepare   注册（打印 email）
//   [编排：psql UPDATE users SET email_verified=true]
//   node r5-wol-e2e.mjs getcode   登录 + GET /agents/default（打印 pairing_code）
//   [编排：--profile agent up -d agent（注入 pairing code）]
//   node r5-wol-e2e.mjs measure   轮询 public_key → 建 device（真加密 MAC）→
//                                  R5_ROUNDS 轮 wake→wakes 往返，报告分位数
import { webcrypto } from 'node:crypto'

const BASE_URL = process.env.BASE_URL || 'http://app:8443'
const ROUNDS = parseInt(process.env.R5_ROUNDS || '30', 10)
const EMAIL = process.env.R5_EMAIL || `r5-${Date.now()}@load.wakewake.local`
const PASSWORD = 'TestPass123!'
const MAC = 'AA:BB:CC:00:11:22'

async function api(path, { method = 'get', token, body } = {}) {
  const res = await fetch(`${BASE_URL}/api/v1${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  const text = await res.text()
  let json = null
  try { json = JSON.parse(text) } catch { /* 非 JSON */ }
  if (!res.ok) throw new Error(`${method} ${path} -> ${res.status}: ${text.slice(0, 200)}`)
  return json
}

async function login() {
  const r = await api('auth/login', { method: 'post', body: { email: EMAIL, password: PASSWORD } })
  return r.access_token
}

// RSA-OAEP+SHA-256 加密 MAC（与前端一致；agent 侧 crypto.rs 解密）
async function encryptMac(publicKeyB64, mac) {
  const der = Buffer.from(publicKeyB64, 'base64')
  const key = await webcrypto.importKey('spki', der, { name: 'RSA-OAEP', hash: 'SHA-256' }, false, ['encrypt'])
  const cipher = await webcrypto.encrypt({ name: 'RSA-OAEP' }, key, Buffer.from(mac, 'utf8'))
  return Buffer.from(cipher).toString('base64')
}

function pct(sorted, p) {
  if (sorted.length === 0) return 'N/A'
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]
}

const [cmd] = process.argv.slice(2)

// register 需 PoW（ui-ux-risk-control §8.6）：GET challenge → 求 nonce → 提交。
// 求解口径与 frontend/src/lib/pow.ts 一致：sha256(challenge + nonce) hex 前缀 N 个 '0'。
async function solvePow() {
  // app 重建后偶发瞬时 404（实测教训）：3 次重试
  // 冷启动窗口实测为 11s 503（Caddy 上游健康检查）；历史还见过 404——
  // 一律重试 60s，只认 200
  let ch
  for (let i = 0; ; i++) {
    try {
      ch = await api('pow/challenge')
      if (ch && ch.id) break
      throw new Error('bad challenge body')
    } catch (e) { if (i >= 19) throw e; await new Promise((r) => setTimeout(r, 3000)) }
  }
  const { createHash } = await import('node:crypto')
  const prefix = '0'.repeat(ch.difficulty)
  for (let nonce = 0; ; nonce++) {
    const hex = createHash('sha256').update(ch.challenge + nonce.toString()).digest('hex')
    if (hex.startsWith(prefix)) return { challenge: ch.id, nonce: nonce.toString() }
  }
}

const fs = await import('node:fs')
if (cmd === 'prepare') {
  // 优先宿主解好的 PoW(undici fetch 对 pow 端点间歇 404,见 run-prod-sim 注释)
  let proof
  try { proof = JSON.parse(fs.readFileSync(process.env.R5_POW_FILE, 'utf8')) } catch { proof = await solvePow() }
  await api('auth/register', { method: 'post', body: { email: EMAIL, password: PASSWORD, ...proof } })
  console.log(EMAIL)
} else if (cmd === 'getcode') {
  const token = await login()
  const agent = await api('agents/default', { token })
  console.log(agent.pairing_code)
} else if (cmd === 'measure') {
  const token = await login()
  // 1. 轮询 public_key（agent 首次 SSE 连接时上报）
  let publicKey = null
  for (let i = 0; i < 60; i++) {
    const agent = await api('agents/default', { token })
    if (agent.public_key) { publicKey = agent.public_key; break }
    await new Promise((r) => setTimeout(r, 2000))
  }
  if (!publicKey) throw new Error('agent public_key 未上报（agent 未连上？）')
  // 2. 建设备（真实加密 MAC —— agent 将真正解密并发 WoL）
  const macEnc = await encryptMac(publicKey, MAC)
  const device = await api('devices', {
    method: 'post',
    token,
    body: { name: 'R5-Target-PC', mac_encrypted: macEnc, mac_display: MAC },
  })
  // 等 agent 收到含新设备的 state snapshot
  await new Promise((r) => setTimeout(r, 6000))
  // 3. wake → 轮询 wakes 落行（agent 执行 + 回报完成）
  const latencies = []
  const failures = []
  for (let i = 0; i < ROUNDS; i++) {
    const t0 = Date.now()
    await api(`devices/${device.did}/wake`, { method: 'post', token })
    let done = false
    for (let poll = 0; poll < 50 && !done; poll++) {
      await new Promise((r) => setTimeout(r, 200))
      const wakes = await api('wakes', { token })
      const items = Array.isArray(wakes) ? wakes : wakes.items || []
      done = items.some((w) => new Date(w.created_at).getTime() >= t0)
    }
    if (done) latencies.push(Date.now() - t0)
    else failures.push(i)
    await new Promise((r) => setTimeout(r, 500))
  }
  latencies.sort((a, b) => a - b)
  const md = `# R5 wake 端到端报告

| 参数 | 值 |
|---|---|
| 用户 / 设备 | ${EMAIL} / ${device.did} |
| 轮次 / 失败 | ${ROUNDS} / ${failures.length}${failures.length ? `（${failures.join(',')}）` : ''} |
| 往返 p50 / p95 / p99 | ${pct(latencies, 50)} / ${pct(latencies, 95)} / ${pct(latencies, 99)} ms |

SLO（specs/testing/load.md §判定方法）：p95 < 3000ms。
链路：POST wake → SSE command → agent RSA 解密 → WoL ×3 → POST /agents/self/wakes → GET /wakes 可见。
`
  console.log(md)
} else {
  console.error('usage: r5-wol-e2e.mjs prepare|getcode|measure')
  process.exit(1)
}
