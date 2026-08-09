// SSE 容量压测 k6+xk6-sse（load.md §6.2）—— 短版用于 CI 验证 slope
// 阶梯 100→500→1000→2000→3000→5000，每级 30s。
// 同时后台采集 RSS（collect-metrics.sh 并行跑）。
import sse from 'k6/x/sse'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const CODES_FILE = __ENV.PAIRING_CODES_FILE

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    ramp_up: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 100 },
        { duration: '30s', target: 500 },
        { duration: '30s', target: 1000 },
        { duration: '30s', target: 2000 },
        { duration: '30s', target: 3000 },
        { duration: '30s', target: 5000 },
        { duration: '30s', target: 0 },
      ],
    },
  },
  thresholds: {
    http_req_failed: [{ threshold: 'rate<0.10', abortOnFail: true }],
    checks: ['rate>0.99'],
  },
}

const __CODES = CODES_FILE ? open(CODES_FILE) : ''

export function setup() {
  const codes = __CODES.split('\n').filter(Boolean)
  if (codes.length < 100) throw new Error(`need >=100 codes, got ${codes.length}`)
  return { pairingCodes: codes }
}

export default function (data) {
  const code = data.pairingCodes[(__VU - 1) % data.pairingCodes.length]
  // sse.open 阻塞直到连接关闭。timeout 让它持 30s（覆盖一个心跳周期）。
  sse.open(
    `${BASE_URL}/api/v1/agents/self/events`,
    { headers: { Authorization: `Bearer ${code}` }, timeout: '30s' },
    function (client) {
      client.on('open', function () {})
      client.on('event', function () {})
      client.on('error', function () {})
    },
  )
}

export function handleSummary(data) {
  return {
    'results/k6-report.json': JSON.stringify(data, null, 2),
    stdout: JSON.stringify({
      vu_max: data.metrics?.vus?.values?.max,
      failed_rate: data.metrics?.http_req_failed?.values?.rate,
      checks_rate: data.metrics?.checks?.values?.rate,
    }, null, 2),
  }
}
