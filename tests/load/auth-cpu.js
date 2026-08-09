// S2 登录 CPU 压测（load.md §6.3）—— 对接 perf-est §5.2。
// constant-arrival-rate 登录风暴，测 auth CPU 临界点（单登录 146ms cost=10）。
// thresholds：auth_latency_ms p95<300 p99<500。
//
// 运行：docker run --rm --network=host -v $(pwd):/scripts \
//        -e BASE_URL=https://localhost:8443 -e LOGIN_RATE=5 \
//        grafana/k6:2.1.0 run /scripts/auth-cpu.js
import http from 'k6/http'
import { check } from 'k6'
import { Trend } from 'k6/metrics'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const authLatency = new Trend('auth_latency_ms')

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    login_storm: {
      executor: 'constant-arrival-rate',
      rate: parseInt(__ENV.LOGIN_RATE || '5'), // 登录/s
      timeUnit: '1s',
      duration: '2m',
      preAllocatedVUs: 50,
      maxVUs: 200,
    },
  },
  thresholds: {
    auth_latency_ms: ['p(95)<300', 'p(99)<500'],
    http_req_failed: ['rate<0.01'],
  },
}

export default function () {
  // 用 seed 用户登录（密码 TestPass123!，固定预计算 hash）
  // 每 VU 取不同 email（seed-NNNNNN@load.wakewake.local）
  const idx = ((__VU - 1) * 1000 + __ITER) % 100000
  const email = `seed-${String(idx).padStart(6, '0')}@load.wakewake.local`
  const start = Date.now()
  const res = http.post(
    `${BASE_URL}/api/v1/auth/login`,
    JSON.stringify({ email, password: 'TestPass123!' }),
    { headers: { 'Content-Type': 'application/json' } },
  )
  authLatency.add(Date.now() - start)
  check(res, {
    'login 200 or 401': (r) => r.status === 200 || r.status === 401,
  })
}
