# 负载与容量测试规格（prod-sim、场景矩阵、判定方法学）

[English](load.md) | 中文

> 本文档是 wakewake 压测的唯一规格:两套测试装置、生产保真环境契约、场景矩阵、seed 工具的数据契约、红线,以及把测量变成"甜点区/极限"表的方法学。操作细节在 `tests/load/` 脚本旁;决策历史在 [`.agents/wrfcs/`](../../.agents/wrfcs/README.zh.md)。

## 1. Two rigs, two questions

| 装置 | 入口 | 环境 | 回答的问题 |
|---|---|---|---|
| 裸机 | `tests/load/run.sh` | e2e compose + `docker-compose.load.yml`:不限 CPU/内存、压测调优 PG(512MB buffers、200 连接)、关限流 | 单连接内存斜率与线性度(10 万外推) |
| prod-sim | `tests/load/run-prod-sim.sh` | e2e compose + `docker-compose.prod-sim.yml`:2c/2g 资源切分、生产 PG GUC、3Mbps 出口整形、限流**开启** | 真实部署目标(2c/2g/3Mbps VPS、藏于免费 Cloudflare)的甜点区与极限 |

裸机装置测不依赖硬件的东西(单连接内存);prod-sim 装置测一切依赖硬件的东西(CPU 天花板、带宽墙、限流校准)。引用为"生产容量"的数字必须来自 prod-sim。

## 2. prod-sim environment contract

```
app:      cpuset "0,1" (LOAD_CPUSET), mem_limit/memswap 1400m, nofile 1M, production image (s6: Caddy + backend)
postgres: cpuset "0,1", mem_limit 550m, GUCs shared_buffers=256MB max_connections=50 effective_cache_size=1GB maintenance_work_mem=128MB synchronous_commit=off
egress:   tc tbf rate 3mbit burst 32kbit latency 400ms (in the app netns, injected by the wan-shaper sidecar; optional netem delay on top)
network:  wakewake-load-net bridge — k6 joins the bridge, never --network=host (host networking bypasses the shaping netns)
frontend: plain-HTTP Caddyfile (docker/Caddyfile) mounted over the e2e TLS variant
limits:   WAKEWAKE_RATE_LIMIT__DISABLED=false by default; LOAD_DISABLE_RATE_LIMIT=1 is an explicit, result-annotating escape hatch for hardware-ceiling probes
PG port:  published on host 15432 (5432 is commonly taken by the dev compose) for seed access
```

不变量:app 容器重启会重建 netns,编排脚本必须在每次重启后重注入整形(R3 依赖此行为);辅助镜像(`wakewake-k6-sse:1.2.1` = k6 v1.2.1 + xk6-sse v0.1.12、`wakewake-wan-shaper` = alpine + iproute2)由 `tests/load/*.Dockerfile` 本地构建,不改生产镜像。

已知保真度损失(接受并记录在此,读数时对照):无 TLS 终结开销(生产由 Cloudflare 终结;只缺 Origin Cert 握手这一小片 CPU);loader 与 SUT 同宿主(用 `WAN_DELAY`,如 40ms 补偿);Cloudflare 边缘未模拟——直连源站等于全缓存 miss 的最坏情形;CFS quota 与两个真实核不完全等同(dev box 上可用 `cpuset` 钉核取更紧的数)。

## 3. Scenario matrix

| ID | 文件 | 形状 | 回答 | 门禁 |
|---|---|---|---|---|
| R1 | `r1-sse-knee.js` | VU 阶梯爬到 `TARGET_CONNS`(步长 `STAIR_STEP`,每级 2m 爬 + 2m 稳) | SSE 连接拐点(内存/fd/CPU) | 建连 p95 < 1s;斜率红线见 §6 |
| R2 | `r2-login-staircase.js` | 到达率阶梯 1→16 登录/s,每级 2m(**限流关**) | bcrypt CPU 天花板(替换 ×2.5 投影) | 拐点 = p99 < 1.5s 且错误 < 0.1% 的最高级 |
| R3 | `r3-reconnect-storm.js` | `CONNS` 满连接;编排脚本在 `R3_RESTART_AFTER` 秒 `docker restart`;VU 复刻 agent 退避算法 | 风暴收敛、3Mbps 打满时长、DB 放大 | 重启窗口后失败归零;重连 p99 < 60s |
| R4 | `r4-dashboard-mix.js` | 仪表盘会话 `SESS_RATE`/min 到达,会话内轮询(devices 5s、agents 5s、integrations 15s、wakes 10s),偶发唤醒 | 真实浏览器负载;限流校准 | 限流开启时 429 为零;按端点 p95 < 300ms |
| R5 | `r5-wol-e2e.mjs` | 真实用户 + 真实 agent 容器 + 真实 RSA 加密 MAC;`R5_ROUNDS` 轮唤醒往返 | 核心业务 SLO | p95 < 3000ms |
| R6 | `r6-soak.js` | 2m 爬坡 → hold(`DURATION`)→ 2m 下降;SSE VU(`SOAK_CONNS`)与仪表盘会话(`SOAK_SESS_RATE`/min)并发 | 泄漏(RSS 漂移)、PG bloat、fd 增长 | 平台期后 RSS 漂移 < 10%;bloat < 20%;429 为零 |
| R7 | `r7-static.js` | 首载到达:HTML + 不可变资产 + 304 再验证 | 静态占用的源站带宽(全 miss 最坏情形) | 信息性:每次访问 KB 对照 375KB/s 预算 |

旧 S 系列仍可运行(`sse-connections.js`、`auth-cpu.js`、`command-qps.js`、`wol-e2e.js`、`api-mixed.js`);S1 是裸机装置旗舰,S3b/S5 被 R5/R4 取代,仅作历史保留。

R4 通过每会话固定一个伪造 `X-Forwarded-For`(走 `trust_proxy` 提取路径)模拟多客户端 IP——否则单个 k6 容器会把所有会话挤进同一个 per-IP 桶;这与生产中 Cloudflare 产生的头部链路相同。

压缩协商(实测,有约束力):Caddy 只在客户端协商(`Accept-Encoding`)时压缩 API JSON 与静态资产,且永不压缩 `text/event-stream`——与生产 Cloudflare 行为一致,因此 SSE 带宽数字无论如何都是线上口径。k6 的 `http` 模块默认不发 `Accept-Encoding`(xk6-sse 反而会发 `gzip`),所以每个 API/静态场景必须显式设置 `Accept-Encoding: gzip`,否则测的是未压缩流量、高估每请求带宽成本(首轮 R4/R7 的实测教训);只报 `gzip` 是因为 k6 自动解压 gzip 但不解压 zstd——生产浏览器可协商 zstd,故 rig 数字略偏保守。

xk6-sse v0.1.12 约束(实测,对所有 SSE 场景有约束力):`sse.open(url, params, handler)` 阻塞到连接关闭或 `params.timeout` 到期——必须传 timeout 让迭代能结束;k6 指标对象与 `check()` 在 SSE 事件回调内不可用(native 回调边界丢失 goja 绑定——`Object has no member`),因此场景在回调里只置局部标志,全部度量在 `open()` 返回之后做;VU 级状态经模块级变量跨迭代保持,R3 借此复刻 agent 退避状态机。

## 4. Seed contract (`wakewake-seed`)

按数量 N:N 个用户(`seed-%06d@load.wakewake.local`,共用一个按服务端 cost 预计算的 `TestPass123!` bcrypt hash)、N 个 agent(真实 `generate_pairing_code()`,去重)、2N 台设备(每用户上限对齐 `MAX_DEVICES_PER_USER`;`mac_encrypted` 是 344 字符 base64 占位——RSA-2048 OAEP 密文的精确 base64 长度——保证 state 快照与风暴带宽保持生产形状)、30% 用户带 bemfa 集成(`--intg-fraction`,config 为 `{"uid":"<344 字符占位>"}`)。COPY 后一致性自检断言 FK 完整与密文形状。seed 不幂等:重跑需要新卷(`compose down -v`)或 `DELETE FROM users WHERE email LIKE 'seed-%'`。

## 5. Judgement methodology

SLO 集(业务口径,不是硬件百分比):唤醒端到端 p95 < 3s;登录 p99 < 1.5s;SSE 建连 p95 < 1s;仪表盘 API p95 < 300ms;错误率 < 0.1%。饱和 = 以下任一:某一资源(CPU/内存/出口/fd)连续 5 分钟 > 70%;任一 SLO 破;错误率 > 0.1%。**拐点(knee)** = 无任何饱和的最高阶梯级;**甜点** = 拐点 × 0.7(留 30% 余量);**极限**按资源维度分别报告(CPU、带宽、内存各自到 100% 的位置),绝不用单一数字。带宽证据来自 wan-sampler 的 5s `tc -s qdisc` 快照(bytes 差分 = 出口速率,drops = 整形压力),不只看 k6 吞吐。

## 6. Red lines

阈值唯一真源是 `tests/load/analyze-regression.py` 的 `THRESHOLD_*` 常量:单连接 RSS 斜率 < 4.0KB(理论 1.2–1.5KB + tokio task header + allocator 开销;实测 3.073KB)、R² ≥ 0.95、10 万外推落在 110–500MB(实测 315MB)。CPU 斜率是信息性的(应近水平;陡升是拐点信号,不是自动失败)。`--check-only` 是面向 CI 的门禁,读 `results/regression.md`。改阈值意味着同一变更里同时改这里常量与本规格。

## 7. CI

`.github/workflows/load.yml`(仅手动 `workflow_dispatch`——压测不随 push 跑):构建辅助镜像,在 4-vCPU runner 上以所选场景跑 `run-prod-sim.sh`(seed 经 `SEED_DOCKER_IMAGE` 在 `rust:1.95-slim` 容器内编译),上传 `tests/load/results/` 产物,并跑红线检查。全量容量工作留在 dev box:`run-full-matrix.sh` 顺序执行 seed 一次 → R7 → R3 → R1 → R4 → R5 → R2(限流关)→ R6 浸泡,loader 钉离 SUT 专核(`K6_CPUSET`),附加 30ms±5 netem 单跳,每次跑追加 `results/manifest.csv`;CI 覆盖接线回归与小规模校准。

## 8. Cloudflare free-tier assumptions baked into the numbers

30s SSE 心跳覆盖边缘空闲超时(100–120s)并容忍一次丢拍;agent 流量走代理(不走 DNS-only 旁路——源站 IP 保持隐藏);静态分发依赖 `docker/caddy/routes.caddy` 的源站响应头(`/_next/static/*` immutable 1 年;HTML 外壳 `max-age=0, must-revalidate`)加一条给 HTML 外壳短 Edge TTL 的 Cache Rule——源站静态出口因此收敛到 miss 率 × R7 每次访问体积。Cloudflare 免费版到源站是否走 HTTP/1.1(预期情形——HTTP/2-to-origin 是付费功能)只在极端 agent 数量下才要紧:每连接 TCP 状态替代 h2c 多路复用,这正是内存数字必须读 prod-sim 装置、不能假设自裸机装置的原因。
