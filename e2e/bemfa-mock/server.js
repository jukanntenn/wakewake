// Bemfa HTTP API mock（E2E 专用，device-sync-v3 有状态版）。
// agent 通过 WAKEWAKE_BEMFA__API_BASE 指向本服务。覆盖 agent 用到的 5 个端点：
//   POST /v1/createTopic           → 记录 topic（幂等：已存在返 40006 视作成功）
//   POST /vs/web/v2/createTopic    → 同上（v2 凭证）
//   POST /v1/deleteTopic           → 删除 topic
//   POST /va/modifyName            → 改 topic 昵称
//   GET  /vb/api/v2/allTopic       → 返回当前所有 topic（有状态，支撑漂移检测）
//
// 有状态设计（device-sync-v3 §9）：维护内存 Map<topic, {name}>，allTopic 返回当前态。
// 这样 e2e 可模拟"用户在云端删/改 topic"→ agent 下一轮 allTopic 观测到差异 → 漂移检测。
//
// 调试端点（仅供 e2e 测试用，非巴法真实接口）：
//   GET  /__debug/topics           → 返回当前内存 topic 表（断言用）
//   POST /__debug/reset            → 清空 topic 表（测试隔离用）
//   POST /__debug/delete           → body {topic} 模拟"用户在云端删 topic"（触发漂移 deleted）
//   POST /__debug/rename           → body {topic, name} 模拟"用户在云端改名"（触发漂移 renamed）
//
// 请求体记录到 /tmp/bemfa-mock.log 便于调试。

import { createServer } from 'node:http'
import { appendFileSync } from 'node:fs'

const PORT = Number(process.env.PORT ?? 8080)
const LOG = '/tmp/bemfa-mock.log'

// 有状态 topic 存储：topic 名 → { name, time, unix }
const topics = new Map()

function log(line) {
  try {
    appendFileSync(LOG, `${new Date().toISOString()} ${line}\n`)
  } catch {
    // 记录失败不影响 mock 响应
  }
}

function okJson(data) {
  return { code: 0, message: 'OK', data: data ?? 0 }
}

function topicItem(topic) {
  const entry = topics.get(topic)
  return {
    topic,
    msg: 'off',
    name: entry?.name ?? '',
    online: true,
    time: entry?.time ?? new Date().toISOString(),
    unix: entry?.unix ?? Math.floor(Date.now() / 1000),
  }
}

const server = createServer((req, res) => {
  let body = ''
  req.on('data', (c) => (body += c))
  req.on('end', () => {
    const url = new URL(req.url, `http://${req.headers.host}`)
    log(`${req.method} ${url.pathname}${url.search} body=${body}`)

    let parsed = {}
    try {
      parsed = body ? JSON.parse(body) : {}
    } catch {
      // 解析失败按空对象处理
    }

    // ---- 调试端点（e2e 测试专用）----
    if (req.method === 'GET' && url.pathname === '/__debug/topics') {
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ topics: Array.from(topics.keys()), detail: Object.fromEntries(topics) }))
      return
    }
    if (req.method === 'POST' && url.pathname === '/__debug/reset') {
      topics.clear()
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson(0)))
      return
    }
    if (req.method === 'POST' && url.pathname === '/__debug/delete') {
      // 模拟"用户在云端删 topic"（触发 agent 漂移 deleted）
      const { topic } = parsed
      if (topic) topics.delete(topic)
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson(0)))
      return
    }
    if (req.method === 'POST' && url.pathname === '/__debug/rename') {
      // 模拟"用户在云端改名"（触发 agent 漂移 renamed）
      const { topic, name } = parsed
      if (topic && topics.has(topic)) {
        const entry = topics.get(topic)
        entry.name = name
        entry.time = new Date().toISOString()
        entry.unix = Math.floor(Date.now() / 1000)
      }
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson(0)))
      return
    }

    // ---- 巴法真实接口（agent 调用）----
    if (req.method === 'GET' && url.pathname === '/vb/api/v2/allTopic') {
      // 有状态：返回当前内存中所有 topic
      // 真实结构：topic 数组嵌套在 data.data（对齐巴法生产 API，非 .local 旧文档）
      const data = Array.from(topics.keys()).map(topicItem)
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ code: 0, msg: 'success', data: { data } }))
      return
    }
    if (req.method === 'POST' && url.pathname === '/v1/deleteTopic') {
      const { topic } = parsed
      if (topic) topics.delete(topic)
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson(0)))
      return
    }
    if (req.method === 'POST' && url.pathname === '/va/modifyName') {
      const { topic, name } = parsed
      if (topic && topics.has(topic)) {
        const entry = topics.get(topic)
        entry.name = name
        entry.time = new Date().toISOString()
        entry.unix = Math.floor(Date.now() / 1000)
      }
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson(0)))
      return
    }
    if (
      req.method === 'POST' &&
      (url.pathname === '/v1/createTopic' || url.pathname === '/vs/web/v2/createTopic')
    ) {
      const { topic, name } = parsed
      if (topic && !topics.has(topic)) {
        topics.set(topic, {
          name: name ?? '',
          time: new Date().toISOString(),
          unix: Math.floor(Date.now() / 1000),
        })
      }
      // createTopic 巴法响应格式：{code:0, message:..., data:{code:0, message:""}}
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(okJson({ code: 0, message: '' })))
      return
    }

    res.writeHead(404, { 'Content-Type': 'application/json' })
    res.end(JSON.stringify({ code: 40000, message: 'not found' }))
  })
})

server.listen(PORT, () => log(`bemfa mock (stateful) listening on :${PORT}`))
