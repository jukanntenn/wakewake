// R1 SSE 容量拐点（specs/testing/load.md §R1）—— 细粒度阶梯找 knee。
//
// 与 sse-connections.js（粗阶梯 + 断言）互补：本场景按 STAIR_STEP 步进爬到
// TARGET_CONNS，每级 ramp 2min + hold 2min，收敛判据交给分析阶段：
//   knee = 最后一个「建连 p95<1s 且 checks>99% 且 collect-metrics 显示
//          RSS 斜率线性、CPU<70%」的阶梯；甜点 = knee × 0.7。
//
// >50k 段用已验证的斜率外推（analyze-regression.py），真 10 万是 k8s 骨架的未来工作。
//
// 连接模型（xk6-sse v0.1.12 实测语义，specs/testing/load.md §xk6-sse 约束）：
// 一 iteration = 一次连接：open() 阻塞持有；v0.1.12 无 timeout 参数，
// 有界持有靠"心跳到点 client.close()"（server 30s ping，HOLD_MS 取 35s）。
// 度量在 open() 返回后做（SSE 回调内 k6 指标对象不可用）。
// 稳态连接数 ≈ VU 数，每 ~HOLD_MS 一次短重连（连接空窗 ms 级）。
//
// seed 扩展后每 agent 的 state snapshot 含 2 设备 + ~30% bemfa 集成
// （payload ≈ 329B+），建连的带宽口径与生产一致。
//
// 运行：SCENARIO=r1-sse-knee TARGET_CONNS=30000 ./run-prod-sim.sh
import sse from 'k6/x/sse'
import { SharedArray } from 'k6/data'
import { check } from 'k6'
import { Counter, Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
// per-VU 稳定伪造客户端 IP：agent 现实里分布在不同 IP，per-IP 600/min 限流
// 各自计桶（k6 单容器单 IP 会被节流成 429 伪影——R3 首轮实测教训）
const VU_IP = `10.60.${(__VU >> 8) & 255}.${(__VU & 255) || 1}`
const CODES_FILE = __ENV.PAIRING_CODES_FILE
const TARGET = parseInt(__ENV.TARGET_CONNS || '30000')
const STEP = parseInt(__ENV.STAIR_STEP || '5000')
const HOLD_MS = parseInt(__ENV.R1_HOLD_MS || '35000')

const codes = new SharedArray('pairingCodes', function () {
  return (CODES_FILE ? open(CODES_FILE) : '').split('\n').filter(Boolean)
})
const connectLatency = new Trend('r1_connect_latency_ms')
const connectFailures = new Counter('r1_connect_failures')

// 阶梯：0→STEP→2STEP→…→TARGET，每级 2min ramp + 2min hold
const LEVELS = []
for (let n = STEP; n <= TARGET; n += STEP) LEVELS.push(n)
export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    knee: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: LEVELS.map((target) => [
        { duration: '2m', target },
        { duration: '2m', target },
      ]).flat(),
    },
  },
  thresholds: {
    // 延迟不设阈值（拐点判定交给分析阶段），只拦完全失败
    checks: ['rate>0.95'],
  },
}

export function setup() {
  if (codes.length < TARGET) {
    throw new Error(`need >=${TARGET} codes, got ${codes.length}（SEED_COUNT >= TARGET）`)
  }
  return { pairingCodes: codes }
}

export default function (data) {
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
  } catch (e) {
    connectFailures.add(1)
  }
  if (openedAt !== null && resp && resp.status === 200) {
    connectLatency.add(openedAt - attemptStart)
    check(null, { 'sse connected': () => true })
  } else if (resp && resp.status !== 200) {
    connectFailures.add(1) // 429/5xx：xk6-sse 不计 http_req_failed，这里补
  }
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
  const md = `# R1 SSE 拐点报告

| 参数 | 值 |
|---|---|
| 目标连接 / 步长 / 持有窗口 | ${TARGET} / ${STEP} / ${HOLD_MS}ms |
| 建连失败次数 | ${m.r1_connect_failures?.values?.count ?? 0} |
| 建连延迟 p50 / p95 / p99 | ${pct('r1_connect_latency_ms', 'p(50)')} / ${pct('r1_connect_latency_ms', 'p(95)')} / ${pct('r1_connect_latency_ms', 'p(99)')} ms |
| checks rate | ${m.checks?.values?.rate ?? 'N/A'} |

RSS 斜率 / CPU 回归：results/regression.md（analyze-regression.py，斜率红线 4KB/连接）。
knee 判定：见 specs/testing/load.md §judgement。
`
  return {
    '/scripts/results/r1-report.md': md,
    '/scripts/results/r1-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
