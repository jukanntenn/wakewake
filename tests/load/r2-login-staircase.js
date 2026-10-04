// R2 登录阶梯（specs/testing/load.md §R2）—— 实测 bcrypt CPU 天花板。
//
// perf-est §4.1 投影：单登录 ~146ms（cost=10，VPS 投影 ×2.5）→ 2 核饱和 ~13/s。
// 本场景在 2 quota 核上阶梯爬升，找实测拐点替换投影值。
//
// ⚠ 必须关限流运行（auth 端点 per-IP 10/min 会把单 IP 的 k6 卡死）：
//   LOAD_DISABLE_RATE_LIMIT=1 SCENARIO=r2-login-staircase ./run-prod-sim.sh
// 结果只回答"硬件天花板"，不代表限流开启时的对外服务能力。
//
// 阶梯：1 → 3 → 5 → 7 → 10 → 13 → 16 登录/s，每级 2min。
// 拐点判据（分析阶段）：p99<1.5s 且错误率<0.1% 的最高一级；
// 饱和证据：docker stats 的 app CPU%（collect-metrics 采样）抵达 ~100%×2 核。
import http from 'k6/http'
import { check } from 'k6'
import { Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
const SEED_COUNT = parseInt(__ENV.SEED_COUNT || '12000')

// 阶梯可扩展（首轮 16/s 未触顶后用 R2_STAGES=20,24,28,32 续测）
const STAGES = (__ENV.R2_STAGES || '1,3,5,7,10,13,16').split(',').map(Number)
const STAGE_SECS = 120
const authLatency = new Trend('r2_auth_latency_ms')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    staircase: {
      executor: 'ramping-arrival-rate',
      startRate: 1,
      timeUnit: '1s',
      preAllocatedVUs: 40,
      maxVUs: 200,
      stages: STAGES.map((rate) => ({ duration: `${STAGE_SECS}s`, target: rate })),
    },
  },
  thresholds: {
    // 只拦"彻底崩了"；拐点判定在分析阶段按 stage tag 做
    checks: ['rate>0.90'],
  },
}

let testStartAt = 0
export function setup() {
  testStartAt = Date.now()
  return { startAt: testStartAt }
}

// 按 elapsed 计算当前阶梯 tag（供 handleSummary 分级输出）
function stageLabel(elapsedMs) {
  const idx = Math.min(Math.floor(elapsedMs / 1000 / STAGE_SECS), STAGES.length - 1)
  return `${STAGES[idx]}/s`
}

export default function (data) {
  const stage = stageLabel(Date.now() - data.startAt)
  const idx = ((__VU - 1) * 7919 + __ITER * 104729) % SEED_COUNT
  const email = `seed-${String(idx).padStart(6, '0')}@load.wakewake.local`
  const start = Date.now()
  const res = http.post(
    `${BASE_URL}/api/v1/auth/login`,
    JSON.stringify({ email, password: 'TestPass123!' }),
    {
      headers: { 'Content-Type': 'application/json' },
      tags: { name: 'POST /auth/login', stage },
    },
  )
  authLatency.add(Date.now() - start, { stage })
  check(res, {
    'login ok': (r) => r.status === 200,
  })
}

export function handleSummary(data) {
  const rows = STAGES.map((rate) => {
    const t = `r2_auth_latency_ms{stage:${rate}/s}`
    const v = data.metrics[t]?.values
    if (!v) return null
    // 该级 checks 通过率（失败率>0.1% 即破 SLO）
    return `| ${rate}/s | ${v['avg'].toFixed(0)} | ${v['p(50)'].toFixed(0)} | ${v['p(95)'].toFixed(0)} | ${v['p(99)'].toFixed(0)} |`
  })
    .filter(Boolean)
    .join('\n')
  const md = `# R2 登录阶梯报告（限流关闭，硬件天花板）

| 阶梯 | avg | p50 | p95 | p99 (ms) |
|---|---|---|---|---|
${rows}

拐点 = p99<1500ms 且失败率<0.1% 的最高阶梯（对照 collect-metrics 的 app CPU 抵达 ~200%）。
perf-est §4.1 投影值：~146ms/登录 → 2 核饱和 ~13/s。以本表实测为准。
`
  return {
    '/scripts/results/r2-report.md': md,
    '/scripts/results/r2-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
