// SSE 容量压测 smoke（验证基础设施，少量 VU）。
// 用法：docker run --rm --network=host -v $(pwd):/scripts \
//        -e PAIRING_CODES_FILE=/scripts/pairing_codes.txt \
//        grafana/k6:2.1.0 run /scripts/sse-smoke.js
import sse from 'k6/x/sse'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const CODES_FILE = __ENV.PAIRING_CODES_FILE

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    smoke: {
      executor: 'ramped-vus',
      startVUs: 0,
      stages: [
        { duration: '10s', target: 20 },
        { duration: '30s', target: 20 },
        { duration: '10s', target: 0 },
      ],
    },
  },
  thresholds: {
    http_req_failed: [{ threshold: 'rate<0.20', abortOnFail: true }],
    checks: ['rate>0.90'],
  },
}

export function setup() {
  if (!CODES_FILE) throw new Error('PAIRING_CODES_FILE env required')
  const codes = open(CODES_FILE).trim().split('\n').filter(Boolean)
  return { pairingCodes: codes }
}

export default function (data) {
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
  sleep(10)
}

export function handleSummary(data) {
  return {
    'results/smoke-report.json': JSON.stringify(data, null, 2),
    stdout: JSON.stringify(
      {
        vu_max: data.state?.vuCountMax,
        failed_rate: data.metrics?.http_req_failed?.values?.rate,
        checks_rate: data.metrics?.checks?.values?.rate,
      },
      null,
      2,
    ),
  }
}
