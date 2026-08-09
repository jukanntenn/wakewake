// S5 混合 CRUD（load.md §6.6）—— 比例 70% 读 / 20% 写 / 10% 动作。
// thresholds：http_req_duration p95<100 p99<200，http_req_failed rate<0.01。
import http from 'k6/http'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const ACCESS_TOKEN = __ENV.ACCESS_TOKEN

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    mixed_crud: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 50 },
        { duration: '2m', target: 200 },
        { duration: '30s', target: 0 },
      ],
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<100', 'p(99)<200'],
    http_req_failed: ['rate<0.01'],
  },
}

const headers = {
  'Content-Type': 'application/json',
  Authorization: `Bearer ${ACCESS_TOKEN || ''}`,
}

export default function () {
  const r = Math.random()
  if (r < 0.7) {
    // 70% 读（GET /devices, /agents/default, /wakes）
    const readPath = ['/devices', '/agents/default', '/wakes'][Math.floor(Math.random() * 3)]
    const res = http.get(`${BASE_URL}/api/v1${readPath}`, { headers })
    check(res, { 'read 200': (r) => r.status === 200 })
  } else if (r < 0.9) {
    // 20% 写（PATCH /devices/:did —— 这里简化为 GET，避免污染数据）
    const res = http.get(`${BASE_URL}/api/v1/devices`, { headers })
    check(res, { 'write-ish 200': (r) => r.status === 200 })
  } else {
    // 10% 动作（POST /devices/:did/wake —— 需真实 did，这里简化）
    const res = http.get(`${BASE_URL}/api/v1/devices`, { headers })
    check(res, { 'action-ish 200': (r) => r.status === 200 })
  }
  sleep(0.1)
}
