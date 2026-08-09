// S1 SSE 容量压测（load.md §6.2）—— 最高价值场景。
// 阶梯 500→2k→3k→5k VU，每 VU 持 1 SSE 连接。
// thresholds：失败率<10%（abortOnFail）、checks>99%、p95<2000ms。
//
// 红线（load.md §7.6）：单 SSE 连接增量 <2.5KB（RSS 线性回归斜率，analyze-regression.py）。
//
// 运行：docker run --rm --network=host --memory=8g -v $(pwd):/scripts \
//        -e BASE_URL=https://localhost:8443 -e PAIRING_CODES_FILE=/scripts/pairing_codes.txt \
//        grafana/k6:2.1.0 run /scripts/sse-connections.js
import sse from 'k6/x/sse'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const CODES_FILE = __ENV.PAIRING_CODES_FILE

export const options = {
  insecureSkipTLSVerify: true, // Caddy tls internal 自签名
  scenarios: {
    ramp_up: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '1m', target: 500 }, // L1
        { duration: '3m', target: 500 }, // 稳态采集
        { duration: '1m', target: 2000 }, // L2
        { duration: '3m', target: 2000 },
        { duration: '1m', target: 3000 }, // L3（主要验证点）
        { duration: '3m', target: 3000 },
        { duration: '1m', target: 5000 }, // L4（峰值，探测边界）
        { duration: '2m', target: 5000 },
        { duration: '2m', target: 0 }, // 降压
      ],
    },
  },
  thresholds: {
    // 失败率>10%立即停（abortOnFail，load.md §7.5）
    http_req_failed: [{ threshold: 'rate<0.10', abortOnFail: true }],
    checks: ['rate>0.99'],
    http_req_duration: ['p(95)<2000'],
  },
}

export function setup() {
  if (!CODES_FILE) {
    throw new Error('PAIRING_CODES_FILE env required (run seed first)')
  }
  // k6 v1.x: open() must be in init context, not setup. Read codes at top-level.
  const codes = __CODES.split('\n').filter(Boolean)
  if (codes.length < 100) {
    throw new Error(`need >=100 codes, got ${codes.length}`)
  }
  return { pairingCodes: codes }
}

// Read pairing codes at init stage (k6 requires open() in global scope)
const __CODES = CODES_FILE ? open(CODES_FILE) : ''

export default function (data) {
  // 每 VU 唯一 code（__VU 从 1 开始）
  const code = data.pairingCodes[(__VU - 1) % data.pairingCodes.length]
  sse.open(
    `${BASE_URL}/api/v1/agents/self/events`,
    { headers: { Authorization: `Bearer ${code}` } },
    function (client) {
      client.on('open', function () {
        check(true, { 'sse connected': (v) => v === true })
      })
      client.on('event', function () {})
      client.on('error', function (e) {
        console.error('SSE error:', e.error())
      })
    },
  )
  sleep(30) // 持连接 30s（覆盖心跳周期）
}

export function handleSummary(data) {
  return {
    'results/report.json': JSON.stringify(data, null, 2),
    stdout: asciiSummary(data),
    'results/summary.md': markdownSummary(data),
  }
}

function asciiSummary(data) {
  const m = data.metrics
  return `
===== S1 SSE 容量压测摘要 =====
VU 峰值: ${data.state?.vuCountMax || 'N/A'}
http_req_failed: ${m.http_req_failed?.values?.rate?.toFixed(4) || 'N/A'}
checks rate: ${m.checks?.values?.rate?.toFixed(4) || 'N/A'}
http_req_duration p95: ${m.http_req_duration?.values?.['p(95)']?.toFixed(0) || 'N/A'}ms
data_received: ${(m.data_received?.values?.count / 1024 / 1024)?.toFixed(2) || 'N/A'} MB
data_sent: ${(m.data_sent?.values?.count / 1024 / 1024)?.toFixed(2) || 'N/A'} MB
`
}

function markdownSummary(data) {
  const m = data.metrics
  return `# S1 SSE 容量压测摘要

| 指标 | 值 |
|---|---|
| VU 峰值 | ${data.state?.vuCountMax || 'N/A'} |
| http_req_failed | ${m.http_req_failed?.values?.rate?.toFixed(4) || 'N/A'} |
| checks rate | ${m.checks?.values?.rate?.toFixed(4) || 'N/A'} |
| http_req_duration p95 | ${m.http_req_duration?.values?.['p(95)']?.toFixed(0) || 'N/A'} ms |
| data_received | ${(m.data_received?.values?.count / 1024 / 1024)?.toFixed(2) || 'N/A'} MB |
| data_sent | ${(m.data_sent?.values?.count / 1024 / 1024)?.toFixed(2) || 'N/A'} MB |

详见 regression.md（线性回归 + 红线断言）。
`
}
