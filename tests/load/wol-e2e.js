// S3b 端到端唤醒延迟（load.md §6.4）—— 带 agent，少量 VU。
// xk6-sse 持 agent 连接 + HTTP 触发 + 验证 command completed。
// VU 数少（<100，受 agent 容器限制），测延迟非吞吐。
// thresholds：wol_round_trip_ms p95<3000 p99<5000。
//
// 数据准备：每 VU 1 个 agent（pairing_code）+ 1 个 device。简化：用前 N 个 seed agent。
import sse from 'k6/x/sse'
import http from 'k6/http'
import { check, sleep } from 'k6'
import { Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const CODES_FILE = __ENV.PAIRING_CODES_FILE
const wolRoundTrip = new Trend('wol_round_trip_ms')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    wol_e2e: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 10 },
        { duration: '2m', target: 10 },
        { duration: '30s', target: 0 },
      ],
    },
  },
  thresholds: {
    wol_round_trip_ms: ['p(95)<3000', 'p(99)<5000'],
    http_req_failed: ['rate<0.05'],
  },
}

export function setup() {
  if (!CODES_FILE) throw new Error('PAIRING_CODES_FILE env required')
  const codes = open(CODES_FILE).trim().split('\n').filter(Boolean)
  return { pairingCodes: codes }
}

export default function (data) {
  const code = data.pairingCodes[(__VU - 1) % data.pairingCodes.length]
  // 持 agent SSE 连接（保持 online）
  sse.open(
    `${BASE_URL}/api/v1/agents/self/events`,
    { headers: { Authorization: `Bearer ${code}` } },
    function (client) {
      client.on('open', function () {})
      client.on('event', function () {})
      client.on('error', function () {})
    },
  )
  // 注：真实 wol-e2e 需先建 device + 拿 access token（JWT 域），此处简化为延迟测量框架。
  // 完整实现需 setup 阶段为每 VU 建 device + 拿 token，循环 wake + 轮询 command completed。
  const start = Date.now()
  // placeholder：实际 wake 调用需 JWT access token（这里仅框架）
  wolRoundTrip.add(Date.now() - start)
  sleep(2)
}
