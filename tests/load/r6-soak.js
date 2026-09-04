// R6 浸泡（specs/testing/load.md §R6）—— 泄漏与慢性病：RSS 漂移、fd 增长、PG bloat。
//
// 形态（markpost soak 先例）：2m ramp → 长 hold → 2m 降，双场景并发：
//   soak_sse  稳态 SSE 连接（ramping-vus 到 SOAK_CONNS 后持有；连接模型同 R1：
//             心跳到点 client.close() 有界持有 + 立即重连，断开则按 agent 退避）
//   soak_dash 仪表盘会话（constant-arrival-rate SOAK_SESS_RATE/min，会话节奏同 R4）
//
// k6 侧门禁（不 abort，浸泡容忍波动，红线看外部采样）：
//   r6_429 必须为 0（限流开启）；SSE 建连 p95 < 1s。
// 慢性病门禁（外部，分析阶段判）：
//   RSS 平台期后漂移 <10%（metrics-samples.csv）；fd 数无增长（docker exec 计数）；
//   PG bloat <20%（pg-health-checks.sql）。
//
// 运行（全矩阵驱动自动最后执行）：
//   SCENARIO=r6-soak SOAK_CONNS=5000 SOAK_SESS_RATE=6 DURATION=8h ./run-prod-sim.sh
import sse from 'k6/x/sse'
import { SharedArray } from 'k6/data'
import http from 'k6/http'
import { sleep } from 'k6'
import { Counter, Rate, Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
// per-VU 稳定伪造客户端 IP：agent 现实里分布在不同 IP，per-IP 600/min 限流
// 各自计桶（k6 单容器单 IP 会被节流成 429 伪影——R3 首轮实测教训）
const VU_IP = `10.60.${(__VU >> 8) & 255}.${(__VU & 255) || 1}`
const CODES_FILE = __ENV.PAIRING_CODES_FILE
const SEED_COUNT = parseInt(__ENV.SEED_COUNT || '12000')
const SOAK_CONNS = parseInt(__ENV.SOAK_CONNS || '5000')
const SOAK_SESS_RATE = parseFloat(__ENV.SOAK_SESS_RATE || '6') // 仪表盘会话/分钟
const HOLD_MS = parseInt(__ENV.R1_HOLD_MS || '35000')
const SESSION_SECS = 240

// "8h"/"90m" → 秒
function durSecs(s) {
  const m = /^(\d+)(h|m|s)$/.exec(s || '8h')
  if (!m) throw new Error(`bad DURATION: ${s}`)
  return parseInt(m[1]) * (m[2] === 'h' ? 3600 : m[2] === 'm' ? 60 : 1)
}
const TOTAL = durSecs(__ENV.DURATION)
const HOLD_SECS = Math.max(TOTAL - 240, 60)
const HOLD_STR = `${Math.floor(HOLD_SECS / 60)}m${HOLD_SECS % 60 ? `${HOLD_SECS % 60}s` : ''}`

const codes = new SharedArray('pairingCodes', function () {
  return (CODES_FILE ? open(CODES_FILE) : '').split('\n').filter(Boolean)
})
const connectLatency = new Trend('r6_connect_latency_ms')
const reconnectDelay = new Trend('r6_reconnect_delay_ms', true)
const rateLimited = new Counter('r6_429')
const sessionOk = new Rate('r6_session_ok')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    soak_sse: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: SOAK_CONNS },
        { duration: HOLD_STR, target: SOAK_CONNS },
        { duration: '2m', target: 0 },
      ],
      gracefulRampDown: '30s',
      exec: 'soakSseFn',
    },
    soak_dash: {
      executor: 'constant-arrival-rate',
      rate: Math.max(1, Math.round(SOAK_SESS_RATE)),
      timeUnit: '1m',
      startTime: '2m',
      duration: `${HOLD_STR}`,
      preAllocatedVUs: 20,
      maxVUs: 80,
      exec: 'soakDashFn',
    },
  },
  thresholds: {
    'http_req_duration{name:GET /devices}': ['p(95)<300'],
    r6_429: [{ threshold: 'count == 0' }],
  },
}

export function setup() {
  if (codes.length < SOAK_CONNS) {
    throw new Error(`need >=${SOAK_CONNS} codes, got ${codes.length}`)
  }
  return { pairingCodes: codes }
}

function fakeIp() {
  return `10.70.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 254) + 1}`
}

// ---- SSE 分支（连接模型同 R1/R3，xk6-sse 约束见 spec）----
let prevDropAt = null
let backoffMs = 5000

function soakSse(data) {
  const code = data.pairingCodes[(__VU - 1) % data.pairingCodes.length]
  const attemptStart = Date.now()
  const holdUntil = attemptStart + HOLD_MS
  let openedAt = null
  let resp = null
  try {
    resp = sse.open(
      `${BASE_URL}/api/v1/agents/self/events`,
      { headers: { Authorization: `Bearer ${code}`, 'X-Forwarded-For': VU_IP } },
      function (client) {
        client.on('open', function () {
          openedAt = Date.now()
        })
        client.on('event', function () {
          if (Date.now() >= holdUntil) {
            try {
              client.close()
            } catch (e) { /* 已关 */ }
          }
        })
        client.on('error', function () {
          try {
            client.close()
          } catch (e) { /* 已关 */ }
        })
      },
    )
  } catch (e) { /* 断连在浸泡内自行退避重连 */ }
  if (openedAt !== null && resp && resp.status === 200) {
    connectLatency.add(openedAt - attemptStart)
    if (prevDropAt !== null) reconnectDelay.add(openedAt - prevDropAt)
  } else if (resp && resp.status === 429) {
    rateLimited.add(1)
  }
  prevDropAt = Date.now()
  if (openedAt !== null && resp && resp.status === 200) backoffMs = 5000 // 429 不重置退避（同 R3 教训）
  const waitMs = Math.min(backoffMs * (0.5 + Math.random()), 300000)
  backoffMs = Math.min(backoffMs * 2, 300000)
  if (waitMs > 0) sleep(waitMs / 1000)
}

// ---- 仪表盘会话分支（节奏同 R4）----
function api(token, xff, path, name) {
  const res = http.get(`${BASE_URL}/api/v1${path}`, {
    headers: {
      Authorization: `Bearer ${token}`,
      'X-Forwarded-For': xff,
      'Accept-Encoding': 'gzip', // 浏览器口径（同 R4 教训）
    },
    tags: { name },
  })
  if (res.status === 429) rateLimited.add(1)
  return res.status
}

function soakDash() {
  const idx = ((__VU - 1) * 7919 + __ITER * 104729) % SEED_COUNT
  const email = `seed-${String(idx).padStart(6, '0')}@load.wakewake.local`
  const xff = fakeIp()
  const loginRes = http.post(
    `${BASE_URL}/api/v1/auth/login`,
    JSON.stringify({ email, password: 'TestPass123!' }),
    { headers: { 'Content-Type': 'application/json', 'X-Forwarded-For': xff, 'Accept-Encoding': 'gzip' }, tags: { name: 'POST /auth/login' } },
  )
  if (loginRes.status !== 200) {
    sessionOk.add(false)
    return
  }
  const token = loginRes.json('access_token')
  const deadline = Date.now() + SESSION_SECS * 1000
  let nextDevices = Date.now()
  let nextAgents = Date.now() + 1000
  let nextIntegrations = Date.now() + 2000
  let nextWakes = Date.now() + 3000
  let allOk = true
  while (Date.now() < deadline) {
    const next = Math.min(nextDevices, nextAgents, nextIntegrations, nextWakes)
    const waitMs = next - Date.now()
    if (waitMs > 0) sleep(Math.min(waitMs, 5000) / 1000)
    const now = Date.now()
    if (now >= nextDevices) {
      if (api(token, xff, '/devices', 'GET /devices') !== 200) allOk = false
      nextDevices = now + 5000
    } else if (now >= nextAgents) {
      if (api(token, xff, '/agents/default', 'GET /agents/default') !== 200) allOk = false
      nextAgents = now + 5000
    } else if (now >= nextIntegrations) {
      if (api(token, xff, '/integrations', 'GET /integrations') !== 200) allOk = false
      nextIntegrations = now + 15000
    } else if (now >= nextWakes) {
      if (api(token, xff, '/wakes', 'GET /wakes') !== 200) allOk = false
      nextWakes = now + 10000
    }
  }
  sessionOk.add(allOk)
}

export function soakSseFn(data) {
  soakSse(data)
}

export function soakDashFn() {
  soakDash()
}

export function handleSummary(data) {
  const m = data.metrics
  const pct = (name, p) => {
    const v = data.metrics[name]?.values
    if (!v) return 'N/A'
    if (p === 'p(50)') return v.med?.toFixed?.(0) ?? 'N/A'
    if (p === 'p(99)' && v['p(99)'] === undefined) return v['p(95)']?.toFixed?.(0) ?? 'N/A'
    return v[p]?.toFixed?.(0) ?? 'N/A'
  }
  const md = `# R6 浸泡报告

| 参数 | 值 |
|---|---|
| SSE 连接 / 会话速率 / 总时长 | ${SOAK_CONNS} / ${SOAK_SESS_RATE}/min / ${__ENV.DURATION || '8h'} |
| 429 计数（限流开启必须为 0） | ${m.r6_429?.values?.count ?? 0} |
| 会话成功率 | ${((m.r6_session_ok?.values?.rate ?? 0) * 100).toFixed(2)}% |
| SSE 建连 p50 / p95 | ${pct('r6_connect_latency_ms', 'p(50)')} / ${pct('r6_connect_latency_ms', 'p(95)')} ms |
| SSE 重连 delay p90 | ${pct('r6_reconnect_delay_ms', 'p(90)')} ms |

慢性病门禁（外部采样判，见 specs/testing/load.md §R6）：
- RSS 平台期后漂移 <10%：results/metrics-samples.csv（首尾 30min 均值对比）
- fd 无增长：docker exec e2e-app-1 sh -c 'ls /proc/<pid>/fd | wc -l' 首尾对比
- PG bloat <20%：pg-health-checks.sql
`
  return {
    '/scripts/results/r6-report.md': md,
    '/scripts/results/r6-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
