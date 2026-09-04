// R7 静态资源/带宽基线（specs/testing/load.md §R7）—— 3Mbps 出口被静态流量占用的实测。
//
// prod-sim 直连源站 = CF 全 miss 的最坏情形（CF 缓存规则生效后源站只吃 miss）。
// 每 iteration：GET /（HTML）+ GET 一个 /_next/static 不可变资产 +
// 一次 If-None-Match 条件请求（模拟浏览器再验证 → 304）。
//
// 关键产出不是延迟阈值，而是 data_received 速率（k6 侧）对照 375KB/s 出口上限：
//   N 并发首载 × 页面体积(≈KB) → 打满出口的并发数 = 375 / (页面KB × 到达率)
//
// 运行：SCENARIO=r7-static ./run-prod-sim.sh
import http from 'k6/http'
import { check, sleep } from 'k6'

const BASE_URL = __ENV.BASE_URL || 'http://app:8443'
const PAGE_RATE = parseFloat(__ENV.PAGE_RATE || '30') // 首载/分钟

export const options = {
  insecureSkipTLSVerify: true,
  scenarios: {
    first_loads: {
      executor: 'constant-arrival-rate',
      rate: Math.max(1, Math.round(PAGE_RATE)),
      timeUnit: '1m',
      duration: __ENV.DURATION || '5m',
      preAllocatedVUs: 10,
      maxVUs: 50,
    },
  },
  thresholds: {
    checks: ['rate>0.99'],
  },
}

let assetUrl = null
let etag = null

export function setup() {
  // 从首页 HTML 提取一个不可变静态资产 URL（Next.js 静态导出必带 hash 名）
  const res = http.get(`${BASE_URL}/`, { headers: { 'Accept-Encoding': 'gzip' }, tags: { name: 'GET / (setup)' } })
  const m = /src="(\/_next\/static\/[^"]+\.js)"/.exec(res.body || '')
  return { asset: m ? m[1] : null }
}

export default function (data) {
  const page = http.get(`${BASE_URL}/`, { headers: { 'Accept-Encoding': 'gzip' }, tags: { name: 'GET / (HTML)' } })
  check(page, { 'html 200': (r) => r.status === 200 })
  etag = page.headers['Etag'] || etag

  if (data.asset) {
    const asset = http.get(`${BASE_URL}${data.asset}`, { headers: { 'Accept-Encoding': 'gzip' }, tags: { name: 'GET /_next/static' } })
    check(asset, { 'asset 200': (r) => r.status === 200 })
  }

  if (etag) {
    const reval = http.get(`${BASE_URL}/`, {
      headers: { 'If-None-Match': etag, 'Accept-Encoding': 'gzip' },
      tags: { name: 'GET / (304 revalidate)' },
    })
    check(reval, { 'revalidate 304': (r) => r.status === 304 })
  }
  sleep(1)
}

export function handleSummary(data) {
  const m = data.metrics
  const kb = (name) => {
    const v = m[`data_received{name:${name}}`]?.values?.count
    return v ? (v / 1024).toFixed(0) : 'N/A'
  }
  const total = (m.data_received?.values?.count / 1024 / 1024)?.toFixed?.(1) ?? 'N/A'
  const secs = (Date.now() - 0, 0) || 300
  const md = `# R7 静态资源基线

| 指标 | 值 |
|---|---|
| 首载速率 | ${PAGE_RATE}/min |
| HTML 传输 | ${kb('GET / (HTML)')} KB |
| 资产传输（不可变） | ${kb('GET /_next/static')} KB |
| 304 再验证传输 | ${kb('GET / (304 revalidate)')} KB |
| 总接收 | ${total} MB（k6 侧；同链路同 3Mbps 出口） |

解读：k6 实测吞吐 ≈ 源站出口占用。CF 缓存规则生效后，HTML/资产命中率抬升，
此处数字收敛到 miss 率 × 上表；CF Pages 方案则归零（specs/testing/load.md §R7）。
`
  return {
    '/scripts/results/r7-report.md': md,
    '/scripts/results/r7-report.json': JSON.stringify(data, null, 2),
    stdout: md,
  }
}
