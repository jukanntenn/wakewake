# RFC: Cloudflare edge topology — real client IP, origin lockdown, cache semantics, and SSE backstops

Status: proposed

[English](2026-08-23-cloudflare-edge-topology.md) | 中文

## Problem

prod 以 Cloudflare 免费版全量代理上线（橙云 DNS、Origin Certificate + Full-strict；宿主 systemd Caddy 网关在 `:443` 作为共用 VPS 上唯一对外监听者，反代到仅回环发布的容器——免费套餐只有 443 端口的代理流量有边缘缓存，故源站直接在 443 应答、不走高位端口改写）。该拓扑带来四个缺陷。其一，客户端 IP 链路端到端都是错的：Caddy 默认行为下，后端拿到的"客户端 IP"是 CF 边缘地址（Caddy 用不受信对端整体覆写 X-Forwarded-For），per-IP governor 桶坍缩成 per-PoP 共享桶——压测（R3）实测发现——`login_events.ip_address` 写进的也是 CF 边缘 IP；R3 的补救（`routes.caddy` 里 per-handler 的 `trusted_proxies`）修好了共享桶，却把 Caddy 切到追加式 XFF——最左值由访客提供、可伪造，后端"取 XFF 最左"的推导随之变成对限流与审计数据的攻击面。其二，源站接受直连：持有 VPS IP 的人绕过 CF 即绕过全部边缘防护（限速、WAF、缓存）直达 Caddy。其三，HTML 响应不带 `Cache-Control`，CF 免费版会注入默认 Browser Cache TTL（4 小时）给浏览器——发版后 dashboard 长时间陈旧——而边缘能有效缓存的只有 immutable 资产。其四，可观测性与滥用兜底缺失：后端默认 `TraceLayer` span 是 DEBUG 级且不含 IP（生产 `log.level=info` 下不可见），Caddy 日志把边缘 IP 记作 `client_ip`，没有任何字段携带 `cf-ray` 用于与 CF 侧采样 Security Events 对账，SSE 端点的连接基数有设备配额约束但没有全局上限。

## Proposal

权威客户端 IP 走专用头，不走 XFF 链。CF 恒发送 `CF-Connecting-IP`（恰好一个 IP，即连接的访客）；CF 之后的 XFF 是追加式、最左可伪造。新增 `[server].client_ip_header`（env `WAKEWAKE_SERVER__CLIENT_IP_HEADER`，默认未设）指名该头；`util::client_ip_from_headers` 先读它，再回退既有 XFF/X-Real-IP/TCP 对端推导，且所有消费方共用这一个函数——governor 的 `ClientIpExtractor`、登录审计（`login_events.ip_address`）、新的 TraceLayer span。未设置时，所有非 CF 环境逐字节保持今日语义。该开关只有在接入层仅放行可信反代时才安全，因此它与源站封锁焊接在一起出厂：prod 宿主跑 systemd Caddy 网关（`:443`）以 Origin Certificate 终结 TLS，其 site 块（`wakewake.caddy.j2`，由 `deploy.yml` 从 `group_vars/prod/env.yml` 的 `cloudflare_cidrs` 渲染，真源 `cloudflare.com/ips`）对非 Cloudflare 来源一律 403，并反代到仅回环发布的容器端口；`CF-Connecting-IP` 经网关原样透传给后端。访客 IP 与 `cf-ray` 的权威记录点是后端 `http_request` span——网关自身的访问日志保持默认形态。后端设置经 `config.toml.j2` 的 `server_client_ip_header` 渲染、仅在定义时输出——prod 设它，其他环境永不设。

缓存语义在源站显式化：`routes.caddy` 已对 `/_next/static/*` 发 `Cache-Control: public, max-age=31536000, immutable`，并在本变更集里对 HTML 外壳发 `public, max-age=0, must-revalidate`——浏览器每次导航都协商（对 `file_server` 的 ETag 304），CF 的 4 小时默认 Browser Cache TTL 不再适用于自带指令的响应；外壳的短 Edge TTL Cache Rule（routes.caddy 注释已引用）是之上的可选带宽卸载。可观测性与兜底补齐：后端 TraceLayer span 改为 INFO 级 `http_request`，带 `client_ip`（同一推导）、`cf_ray`、method、URI，响应事件同样 INFO（status + duration）；全局 SSE 连接上限 `[server].max_sse_connections`（默认 2000，0 = 不限制）对超出的 SSE 握手回 503 SERVICE_UNAVAILABLE——连接基数仍由已配对 agent 数约束（同 agent 重连覆盖、不叠加），上限是总量洪泛的兜底；`sse_connections_active` 指标已有，用于对照观测。免费版控制台姿态（`/api/v1/auth/*` 的 1 条限速规则、Bot Fight Mode 关闭——它会挑战 agent SSE 这类非浏览器客户端、Under Attack 作应急开关、源站错误率告警）与上线验证 runbook 落在 [cloudflare.zh.md](../../../../devops/cloudflare.zh.md)。

由此产生一项同步义务：CF CIDR 集合现在出现在三处、必须同动——`docker/caddy/routes.caddy` 的两行 `trusted_proxies`（烤进镜像、全环境生效）与 `group_vars/prod/env.yml` 的 `cloudflare_cidrs`（prod 网关守卫）。还有一对永不可拆的耦合：只有 `trusted_proxies`（routes.caddy）而没有后端 `client_ip_header`，最左 XFF 伪造依然敞开；只有 `client_ip_header` 而没有网关的 `remote_ip` 守卫，直连方就能伪造权威头本身。移除任一侧都是安全决策，不是清理。

## Alternatives considered

**在 Caddy 侧覆写 XFF（`header_up X-Forwarded-For {http.request.header.CF-Connecting-IP}`）**——CF 官方文档的 Caddy 配方。放弃：它必须落在 `routes.caddy`（四个 TLS 变体共享的路由真源），要么把 CF 专属行为拖进自部署/本地/e2e，要么需要 Caddyfile 并不自然支持的 snippet 参数化机制；配置驱动的后端头让 `routes.caddy` 保持零改动、Rust 侧可单测。

**从右向左解析 XFF、跳过可信反代（Caddy `trusted_proxies_strict` 的逻辑）。** 放弃：它要求枚举每一跳反代的地址段，且只有链路与建模完全一致时才正确；单一权威头是"一个事实"，而不是对前缀受攻击者控制的链路做推断。

**全部在 CF 边缘解决（WAF、限速、Under Attack），仓库侧不动。** 放弃：免费版只有 1 条限速规则、固定 10s 窗口，且文档明确处置生效前超额请求仍会到源站；per-IP governor、PoW、锁定与配额才是精确层，而它们依赖正确的客户端 IP——这件事边缘自己恢复不了。

**Caddy 限速模块做源站侧节流。** 放弃：官方发行二进制没有该插件——引入意味着每个环境都换 xcaddy 自建镜像，而 CF 规则 + governor 双层已覆盖同一威胁。

**per-user SSE 上限替代全局上限。** 放弃：每用户的基数就是设备配额（每个已配对 agent 一条活通道、重连覆盖），per-user 上限只是配额的第二个名字；缺的是总量洪泛兜底，一个全局数字就是它。

## Acceptance criteria

- 单测覆盖 `util.rs` 与 `rate_limit.rs` 的头优先级矩阵：权威头压过伪造 XFF 链、头缺失/非法回退 XFF、未配置时保持 XFF 语义、`trust_proxy=false` 忽略一切头；`routes/sse.rs` 的上限测试覆盖 0 不限制、到限拒绝、同 agent 重连不叠加。
- 渲染出的 prod 配置：`config.toml` 带 `client_ip_header = "CF-Connecting-IP"`；test/staging/自部署配置无此键、行为不变。
- [cloudflare.zh.md](../../../../devops/cloudflare.zh.md) §8 runbook 在生产通过：伪造 XFF 后 `login_events.ip_address` 仍是访客真实 IP、直连 VPS IP 得 403、immutable 资产 MISS→HIT 且 `Age` 增长并保留 origin `Cache-Control`、HTML 外壳呈现短 TTL 边缘行为与 `max-age=0, must-revalidate`、API 响应 DYNAMIC 无 `Age`、SSE 连接保持每 30s 一条 `: ping`。
- 后端 `http_request` span 在 `log.level=info` 下可见，带 `client_ip`（来自网关原样透传的 `CF-Connecting-IP`）与 `cf_ray`；网关 site 块对任何非 CF 直连在抵达容器前即回 403。
- `doc_sync.py` 对触及的文档全绿；`wakewake-server` 的 `cargo clippy` 与 `cargo test` 通过。

## Risks

CF CIDR 清单可能与 `cloudflare.com/ips` 漂移；三份拷贝（routes.caddy 两处、env.yml 一处）让漂移成为静默哑火——过期清单会把合法 CF 流量挡在守卫外（403）或信任 CF 已不用的段，缓解是文档化的季度核对（env.yml 注释写明，但无机制强制）。`trusted_proxies` ↔ `client_ip_header` 的耦合只与本 WRFC 的警告同寿；将来某个"求简洁"的清理若移除后端头，就在恰好信任反代的环境里重新打开最左 XFF 伪造。`max_sse_connections` 的 503 与 mailer 未启用等不可用状态共用错误码，agent 无法区分容量耗尽与故障——可接受（两者都该退避），但读仪表盘的运维要知道每次拒绝都有一条 warn 日志。HTML 外壳的 `must-revalidate` 让每次导航都向源站发条件请求（304，小但在 3Mbps 链路上非零）；若实测变热，设计好的泄压阀是 Cache-Rule 的边缘 TTL——不是放松浏览器指令。免费版行为本身是最外层风险：cloudflare.md 记录的控制台姿态在本仓库没有任何门禁强制，控制台漂移（有人开了 Bot Fight Mode、删了限速规则）会静默劣化，直到重跑 §8 runbook 才暴露。
