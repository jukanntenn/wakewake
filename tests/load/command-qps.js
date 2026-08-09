// S3a 命令创建 QPS（load.md §6.4）—— 无 agent，命令 60s 后 expired。
// 纯 HTTP 压 POST /devices/{did}/wake，验证命令创建吞吐。
// thresholds：http_reqs rate>1000，http_req_duration p95<50。
//
// 数据准备：seed 用户需先有设备（device_did）。简化：用 seed-000001 的用户，
// 预先建 1 设备（压测前 setup），所有 VU 对该设备发 wake。
import http from 'k6/http'
import { check } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'https://localhost:8443'
const DEVICE_DID = __ENV.DEVICE_DID // 预建设备的 did
const ACCESS_TOKEN = __ENV.ACCESS_TOKEN // seed-000001 用户的 access token

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    constant_qps: {
      executor: 'constant-arrival-rate',
      rate: parseInt(__ENV.QPS || '500'),
      timeUnit: '1s',
      duration: '1m',
      preAllocatedVUs: 100,
      maxVUs: 1000,
    },
  },
  thresholds: {
    http_reqs: ['rate>1000'],
    http_req_duration: ['p(95)<50'],
    http_req_failed: ['rate<0.05'],
  },
}

export default function () {
  if (!DEVICE_DID || !ACCESS_TOKEN) {
    throw new Error('DEVICE_DID and ACCESS_TOKEN env required')
  }
  const res = http.post(
    `${BASE_URL}/api/v1/devices/${DEVICE_DID}/wake`,
    JSON.stringify({}),
    { headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${ACCESS_TOKEN}` } },
  )
  check(res, {
    'wake 202': (r) => r.status === 202,
  })
}
