// R3 重连风暴（specs/testing/load.md §R3）—— SSE 最恶劣工况，3Mbps 带宽墙实测。
//
// 编排（run-prod-sim.sh）：VU 先建满连接 → R3_RESTART_AFTER 秒后 docker restart app
// （netns 重建，整形重注入）→ 全量断开 → 各 VU 按 agent 同款退避重连。
//
// 连接模型（xk6-sse v0.1.12 实测语义，见 specs/testing/load.md §xk6-sse 约束）：
//   - 一 iteration = 一次连接生命周期：open() 阻塞持有；v0.1.12 无 timeout 参数
//     （被静默忽略），有界持有靠心跳到点 client.close()——server 每 30s 一条
//     ping（以空 data 的 event 送达），HOLD_MS 取 35s 保证至少跨一个心跳；
//   - k6 指标对象在 SSE 回调内不可用（TypeError: no member）——所有度量在
//     open() 返回后的迭代主体里做；回调只置标志/发起 close；
//   - 退避状态（backoffMs / prevDropAt）是 VU 模块级变量，同一 VU 跨迭代保持，
//     复刻 agent 的"健康连接断开 → 重置 base"（P0 修复后行为）。
//
// 核心指标：
//   r3_reconnect_delay_ms  上次断开 → 重连成功（含退避；断开检测 ≤HOLD，见报告脚注）
//   r3_connect_latency_ms  发起连接 → open 事件（建连延迟）
//   r3_connect_failures    建连失败次数（重启窗口内为预期值，风暴后应归零）
//
// 运行：SCENARIO=r3-reconnect-storm CONNS=2000 R3_RESTART_AFTER=180 ./run-prod-sim.sh
import sse from 'k6/x/sse'
import { SharedArray } from 'k6/data'
import { sleep } from 'k6'
import { Counter, Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
// per-VU 稳定伪造客户端 IP：agent 现实里分布在不同 IP，per-IP 600/min 限流
// 各自计桶（k6 单容器单 IP 会被节流成 429 伪影——R3 首轮实测教训）
const VU_IP = `10.60.${(__VU >> 8) & 255}.${(__VU & 255) || 1}`
const CODES_FILE = __ENV.PAIRING_CODES_FILE
const CONNS = parseInt(__ENV.CONNS || '2000')
// 有界持有窗口：>30s 心跳周期（到点后随下一条 ping 关闭）
const HOLD_MS = parseInt(__ENV.R3_HOLD_MS || '35000')

const codes = new SharedArray('pairingCodes', function () {
  return (CODES_FILE ? open(CODES_FILE) : '').split('\n').filter(Boolean)
})
const reconnectDelay = new Trend('r3_reconnect_delay_ms', true)
const connectLatency = new Trend('r3_connect_latency_ms')
const connectFailures = new Counter('r3_connect_failures')
const reconnectSuccesses = new Counter('r3_reconnect_successes')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    storm: {
      executor: 'constant-vus',
      vus: CONNS,
      duration: __ENV.DURATION || '10m',
    },
  },
  thresholds: {
    // 只拦"完全失败"；建连延迟不设阈值——风暴窗口的慢建连就是数据
    // （markpost 先例：容量场景 breaches are the data，门禁在分析阶段）
    checks: ['rate>0.85'],
  },
}

export function setup() {
  if (codes.length < CONNS) {
    throw new Error(`need >=${CONNS} codes, got ${codes.length}（增大 SEED_COUNT）`)
  }
  return { pairingCodes: codes }
}

// ---- VU 模块级状态（同一 VU 跨迭代保持；每 VU = 1 台 agent）----
let prevDropAt = null // 上一次连接断开（open 返回）的时刻
let backoffMs = 5000 // agent 退避：base 5s ×2 max 300s ±50% jitter

export default function (data) {
  const code = data.pairingCodes[(__VU - 1) % data.pairingCodes.length]
  const attemptStart = Date.now()
  const holdUntil = Date.now() + HOLD_MS
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
          // 有界持有：到点随下一条心跳（30s 周期）主动关闭
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
    connectFailures.add(1) // 建连失败（重启窗口 5xx/拒绝连接为预期）
  }
  // open 返回 = 连接关闭（close/EOF/错误）—— 度量在此（回调内不可用 k6 指标）
  if (openedAt !== null && resp && resp.status === 200) {
    connectLatency.add(openedAt - attemptStart)
    reconnectSuccesses.add(1)
    if (prevDropAt !== null) {
      reconnectDelay.add(openedAt - prevDropAt) // drop → 重连成功（含退避）
    }
  } else if (resp && resp.status !== 200) {
    connectFailures.add(1) // 429/5xx：xk6-sse 不计 http_req_failed，这里补
  }
  prevDropAt = Date.now()
  // 健康连接（200 且曾 open）断开 → 重置退避；429/建流失败 → 棘轮增长
  // （首轮矩阵实测教训：openedAt 在 429 响应头上也会置位，不校验状态码会把
  // 限流拒绝循环重置成 5s 高频重试）
  if (openedAt !== null && resp && resp.status === 200) {
    backoffMs = 5000
  }
  const waitMs = Math.min(backoffMs * (0.5 + Math.random()), 300000)
  backoffMs = Math.min(backoffMs * 2, 300000)
  if (waitMs > 0) sleep(waitMs / 1000)
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
  const md = `# R3 重连风暴报告

| 指标 | 值 |
|---|---|
| 连接数（VU） / 持有窗口 | ${CONNS} / ${HOLD_MS}ms |
| 重连成功次数 | ${m.r3_reconnect_successes?.values?.count ?? 'N/A'} |
| 建连失败次数（重启窗口预期非零） | ${m.r3_connect_failures?.values?.count ?? 'N/A'} |
| 重连收敛 delay p50 / p90 / p99 | ${pct('r3_reconnect_delay_ms', 'p(50)')} / ${pct('r3_reconnect_delay_ms', 'p(90)')} / ${pct('r3_reconnect_delay_ms', 'p(99)')} ms |
| 建连延迟 p50 / p95 / p99 | ${pct('r3_connect_latency_ms', 'p(50)')} / ${pct('r3_connect_latency_ms', 'p(95)')} / ${pct('r3_connect_latency_ms', 'p(99)')} ms |

带宽证据：results/wan-qdisc.log（tbf bytes 5s 差分 = 出口速率，对照 375KB/s 上限）。
收敛判据：r3_connect_failures 在重启窗口后归零；reconnect_delay p99 < 60s。
脚注：断开检测延迟上界 = HOLD_MS（连接实际断开最多一个持有窗口后才被感知），
reconnect_delay 相应低估 ≤ ${HOLD_MS}ms；带宽与 CPU 曲线不受此影响。
`
  return {
    '/scripts/results/r3-report.md': md,
    '/scripts/results/r3-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
