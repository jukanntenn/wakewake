# WRFC: 上线阻断级容量修复 —— 全局限流 per-IP 化、SSE 退避重置与 FD 上限

Status: implemented

[English](2026-08-22-launch-blocking-capacity-fixes.md) | 中文

## Problem

上线前容量评审(目标:2c/2g/3Mbps VPS,藏于免费 Cloudflare 之后)以生产流量视角重读热路径,发现三个在极小规模就会发作的缺陷,且没有一个是硬件极限。第一,[middleware/rate_limit.rs](../../../backend/crates/server/src/middleware/rate_limit.rs) 的兜底限流器以 `Global` 为键——全服务器共享一个 120 req/min 的桶,API 路由合并进来的每条路由都在其中:仪表盘轮询约 40 req/min/打开页面(`useDevices` 同步中 3s、`useAgents` 5s、`useIntegrations` 3s、`useWakes` 10s),约 3 个并发页面就会让所有人吃到 429;agent 的 `/agents/self/sync`、甚至 Docker healthcheck(10s)加 Caddy 上游探测(10s)都从这个被饿着的桶里取水。而压测套件的 compose override 里烤死了 `WAKEWAKE_RATE_LIMIT__DISABLED=true`,任何测试都永远看不见这个问题。第二,agent 的重连退避永远无法重置:`connect_and_run` 只返回 `Unauthorized`/`Disconnected`,[agent main.rs](../../../backend/crates/agent/src/main.rs) 的 `Outcome::Connected` 分支是死代码,`backoff.reset()` 不可达——数周健康连接后一次网络抖动也要按已增长的间隔重试,计数在进程生命周期内单调棘轮上升到 300s 封顶。第三,[docker/docker-compose.yml](../../../docker/docker-compose.yml) 与 ansible 模板都没设 `ulimits.nofile`,SSE 容量听天由命于 daemon 默认值(常见宿主 soft 1024——约一千条连接);压测 override 不得不抬到 1M 才跑得起 5k 连接,这本身就是生产同样需要它的证据。

## Decision

全局兜底限流器改为 per-client-IP、600 req/min——与 auth 限流器同款 `ClientIpExtractor` 与 `trust_proxy` 语义,`max_keys` 50,000、GC 间隔 1 分钟。单个仪表盘消耗的 15 倍,合法用户不可达,对单 IP 洪泛仍是硬顶;per-user/per-agent 预算不变。health 路由(`/health`、`/health/maintenance`、`/version`)在兜底层应用**之后**合并,从而豁免:基础设施探测永远不该消耗面向用户的配额,且限流器出问题时 healthcheck 也必须能应答。agent 的 `connect_and_run` 现在跟踪流是否交付过字节;收到数据之后的断开返回 `Outcome::Connected`,主循环据此重置退避并按 base+jitter(2.5–7.5s)重试——单次抖动约 5s 自愈,服务器重启风暴按设计散布在 ±50% jitter 窗口内。未收到任何字节前的失败(连接错误、非 200、即断 EOF)继续棘轮增长退避。两份生产 compose(自部署与 ansible 模板)都设置 `ulimits.nofile` soft/hard 1,048,576,与压测 override 同口径。

## Alternatives considered

**把 Global 桶调大到一个大常数(如 20k/min)。** 败因:它继续惩罚总流量——对全局键而言,合法流量洪峰与洪泛是同一个事件;而 per-IP 正是让滥用模型(一个坏客户端)与合法模型(众多温和客户端)分离的粒度。

**彻底删掉兜底限流器,只留 auth 限流。** 败因:兜底层是唯一覆盖 JWT 保护与 agent M2M 路由的层;删掉后这些端点完全没有速率上限,而 Cloudflare 免费版只有一条限流规则可花——应该花在 `/auth/login`(bcrypt CPU 墙)上,而不是花在应用自己就能提供的通用覆盖上。

**用 layer 内的 axum matcher 豁免 health。** 败因:`Router::layer` 只作用于已合并的路由,把 health 放在挂层之后合并就是零机械的惯用豁免法;matcher 要重新推导路由早已编码的身份。

**收到 HTTP 200 的瞬间就重置退避(而非首字节之后)。** 败因:200 之后紧跟流错误是半开连接的失败形态,按 base 速率重试它会把抖动的上游变成连接风暴;要求交付过字节,意味着只有真正健康过的连接才赢得重置。

**`Connected` 断开零延迟立即重试。** 败因:服务器重启时会面对整个 agent 机群同一瞬间重连;base+jitter 睡眠(2.5–7.5s 展宽)正是退避设计存在的风暴防护,重置恢复的是*间隔*,不是间隔的消失。

**只在 VPS 宿主机上设 FD 上限(systemd override)。** 败因:compose 的 `ulimits` 随部署定义一起走,被包括压测环境在内的所有环境演练,不依赖一次未成文、未必扛过下次 VPS 重建的宿主机手工步骤。

## Consequences

`cargo clippy` 与全 workspace 测试套件(真实 PG)通过;e2e 与集成套件以 `rate_limit.disabled=true` 运行,配额/键的变更不改变其行为,今后由压测套件的限流开启 profile 来演练 per-IP 语义(load override 从此必须在测余量——而非正确性——时才刻意切换)。429 面保持共享错误处理器的 JSON 包络加 `Retry-After`。per-IP 键依赖正确的客户端 IP 传播,而首次全量压测矩阵恰是那次核实——并且它失败了:Caddy(≥2.7 安全行为)会把不受信来源的入站 X-Forwarded-For 改写为直连对端 IP,于是代理后面的所有客户端塌缩进同一个限流桶(生产中即全体用户共享单个 Cloudflare 边缘 IP——上线即 429 风暴);`docker/caddy/routes.caddy` 的两处 reverse_proxy 现在都声明了 `trusted_proxies static`(Cloudflare 全段 + RFC1918,可经 `TRUSTED_PROXIES` env 覆盖),恢复 CF→Caddy 链路与压测 rig per-VU X-Forwarded-For 的首跳键;`max_keys` 50,000 在 IP 伪造洪泛下限制限流器内存,代价是同场洪泛中合法键被驱逐,这正是预期的降级;`Connected` 重置行为按设计让重启后的重连风暴更密(全部 agent 落在 base+jitter 而非错开的已增长间隔)——压测套件的 R3 重连风暴场景存在的意义就是拿这波浪对着 3Mbps 上行实测,本地 perf-est 笔记中提出的状态推送令牌桶仍未实现,是 R3 测出饱和超容忍时的既定缓解手段。
