// R4 仪表盘混合负载（specs/testing/load.md §R4）—— 前端轮询节奏的真实建模。
//
// 每 iteration = 1 个仪表盘会话：登录 → 按前端 hook 节奏轮询 → 偶发唤醒 → 离开。
// 轮询间隔对齐 frontend/src/hooks/：devices 5s、agents 5s、integrations 15s、
// wakes 10s（取稳态与同步中的折中，≈34 req/min/会话）。
//
// 限流开启 profile（prod-sim 默认）：k6 单容器单 IP 会挤爆 per-IP 600/min，
// 故每会话固定一个伪造 X-Forwarded-For（trust_proxy=true 口径）模拟多客户端；
// r4_rate_limited 应为零 —— 这是 R4 的第一门禁。
//
// 核心指标：
//   按 name tag 的 http_req_duration（per-endpoint p50/p95/p99）
//   r4_rate_limited（429 计数，限流开启时必须为 0）
//
// 运行：SCENARIO=r4-dashboard-mix ./run-prod-sim.sh                    # 限流开启（校准）
//       LOAD_DISABLE_RATE_LIMIT=1 SCENARIO=r4-dashboard-mix SESS_RATE=240 ./run-prod-sim.sh  # 天花板
import http from 'k6/http'
import { check, sleep } from 'k6'
import { Counter, Rate } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
const SESS_RATE = parseFloat(__ENV.SESS_RATE || '36') // 会话/分钟（默认 ≈ 早高峰 180 会话/5min）
const DURATION = __ENV.DURATION || '10m'
const SESSION_SECS = parseFloat(__ENV.SESSION_SECS || '240')
const WAKE_PROB = parseFloat(__ENV.WAKE_PROB || '0.3')
const SEED_COUNT = parseInt(__ENV.SEED_COUNT || '12000')

const rateLimited = new Counter('r4_rate_limited')
const sessionOk = new Rate('r4_session_ok')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    dashboards: {
      executor: 'constant-arrival-rate',
      rate: Math.max(1, Math.round(SESS_RATE)),
      timeUnit: '1m',
      duration: DURATION,
      // 并发会话 ≈ rate/60 × SESSION_SECS；留 2 倍余量
      preAllocatedVUs: Math.max(20, Math.ceil((SESS_RATE / 60) * SESSION_SECS)),
      maxVUs: Math.max(60, Math.ceil(((SESS_RATE / 60) * SESSION_SECS) * 2)),
    },
  },
  thresholds: {
    'http_req_duration{name:GET /devices}': ['p(95)<300'],
    'http_req_duration{name:GET /agents/default}': ['p(95)<300'],
    'http_req_duration{name:GET /integrations}': ['p(95)<300'],
    'http_req_duration{name:GET /wakes}': ['p(95)<300'],
    r4_session_ok: ['rate>0.95'],
  },
}

// 会话内稳定的伪造客户端 IP（10.70.0.0/16 测试段）
function fakeIp() {
  return `10.70.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 254) + 1}`
}

function api(token, xff, method, path, name, body) {
  const params = {
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'X-Forwarded-For': xff,
      // 浏览器口径压缩协商（k6 http 默认不发 AE——实测未压缩流量会高估带宽成本；
      // 只报 gzip：k6 自动解压 gzip，zstd 不解压）。生产浏览器可协商 zstd，更小。
      'Accept-Encoding': 'gzip',
    },
    tags: { name },
  }
  const res =
    method === 'get'
      ? http.get(`${BASE_URL}/api/v1${path}`, params)
      : http.post(`${BASE_URL}/api/v1${path}`, body || '', params)
  if (res.status === 429) rateLimited.add(1)
  return res
}

export default function () {
  // 每 VU-iteration 一个独立 seed 用户（登录 + 全会话持有 token）
  const idx = ((__VU - 1) * 7919 + __ITER * 104729) % SEED_COUNT
  const email = `seed-${String(idx).padStart(6, '0')}@load.wakewake.local`
  const xff = fakeIp()

  const loginRes = http.post(
    `${BASE_URL}/api/v1/auth/login`,
    JSON.stringify({ email, password: 'TestPass123!' }),
    { headers: { 'Content-Type': 'application/json', 'X-Forwarded-For': xff, 'Accept-Encoding': 'gzip' }, tags: { name: 'POST /auth/login' } },
  )
  if (!check(loginRes, { 'login 200': (r) => r.status === 200 })) {
    sessionOk.add(false)
    return
  }
  const token = loginRes.json('access_token')

  const deadline = Date.now() + SESSION_SECS * 1000
  // 各流下次到期时刻（前端 hook 节奏的折中口径）
  let nextDevices = Date.now()
  let nextAgents = Date.now() + 1000
  let nextIntegrations = Date.now() + 2000
  let nextWakes = Date.now() + 3000
  let deviceList = []
  const wakeAt = WAKE_PROB > 0 && Math.random() < WAKE_PROB
    ? Date.now() + Math.random() * SESSION_SECS * 1000
    : Infinity
  let wakeDone = false
  let allOk = true

  while (Date.now() < deadline) {
    const next = Math.min(nextDevices, nextAgents, nextIntegrations, nextWakes, wakeAt)
    const waitMs = next - Date.now()
    if (waitMs > 0) sleep(Math.min(waitMs, 5000) / 1000)
    const now = Date.now()
    if (now >= nextDevices) {
      const r = api(token, xff, 'get', '/devices', 'GET /devices')
      if (r.status === 200) deviceList = r.json() || []
      else allOk = false
      nextDevices = now + 5000
    } else if (now >= nextAgents) {
      if (api(token, xff, 'get', '/agents/default', 'GET /agents/default').status !== 200) allOk = false
      nextAgents = now + 5000
    } else if (now >= nextIntegrations) {
      if (api(token, xff, 'get', '/integrations', 'GET /integrations').status !== 200) allOk = false
      nextIntegrations = now + 15000
    } else if (now >= nextWakes) {
      if (api(token, xff, 'get', '/wakes', 'GET /wakes').status !== 200) allOk = false
      nextWakes = now + 10000
    } else if (now >= wakeAt && !wakeDone) {
      // 有设备才唤醒（seed 未造设备时自动跳过；seed 扩展后生效）
      if (Array.isArray(deviceList) && deviceList.length > 0) {
        const did = deviceList[0].id
        const r = api(token, xff, 'post', `/devices/${did}/wake`, 'POST /devices/{did}/wake')
        if (r.status !== 202) allOk = false
      }
      wakeDone = true
    }
  }
  sessionOk.add(allOk)
}

export function handleSummary(data) {
  const m = data.metrics
  const row = (name) => {
    const v = m[`http_req_duration{name:${name}}`]?.values
    return v ? `| ${name} | ${v['p(50)'].toFixed(0)} | ${v['p(95)'].toFixed(0)} | ${v['p(99)'].toFixed(0)} |` : null
  }
  const rows = ['GET /devices', 'GET /agents/default', 'GET /integrations', 'GET /wakes', 'POST /devices/{did}/wake', 'POST /auth/login']
    .map(row)
    .filter(Boolean)
    .join('\n')
  const md = `# R4 仪表盘混合负载报告

| 参数 | 值 |
|---|---|
| 会话速率 / 时长 / 会话时长 | ${SESS_RATE}/min / ${DURATION} / ${SESSION_SECS}s |
| 唤醒概率 | ${WAKE_PROB} |
| 429 计数（限流开启时必须为 0） | ${m.r4_rate_limited?.values?.count ?? 0} |
| 会话成功率 | ${(m.r4_session_ok?.values?.rate * 100)?.toFixed?.(2) ?? 'N/A'}% |

## 按端点延迟（ms）

| 端点 | p50 | p95 | p99 |
|---|---|---|---|
${rows}
`
  return {
    '/scripts/results/r4-report.md': md,
    '/scripts/results/r4-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
