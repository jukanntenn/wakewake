# RFC: OTLP 可观测接线 —— 遥测推送到运维观测栈，面板与告警作为仓库工件

Status: implemented

[English](2026-10-10-otlp-observability-wiring.md) | 中文

## 问题

后端早已说 OpenTelemetry：`observability` 在设了 `OTEL_EXPORTER_OTLP_ENDPOINT` 时初始化 traces/logs/metrics 的 OTLP 导出器，否则退回本地 JSONL 文件；`observability::metrics` 定义了完整的仪表清单。但从未有部署设过这个端点，于是每个环境都跑在文件模式：遥测只是各宿主机上无人阅读的轮转文件，没有跨环境视图、没有超出磁盘的保留期、没有服务质量的告警。与此同时，一套运维持有的观测栈（共享 collector 扇出到 Jaeger、VictoriaMetrics、VictoriaLogs，Grafana 做唯一人类 UI）已经跑在家用 NAS 上，且每个环境都可达——一扇带按生产者 bearer-token 鉴权的 OTLP 前门。

接线过程中还暴露一个缺口：`http_request_duration_seconds` 直方图声明了却没有任何记录点，任何 HTTP 面板或告警查询的都是一个永不存在的指标。

## 决策

只动部署接线——后端保持现有 OTel 双模式，由部署层按环境打开；不引入新依赖。

- **env 注入** —— `templates/docker-compose.yml.j2` 渲染三个容器 env，守卫在 `otel_otlp_endpoint`（环境变量）与 `otel_otlp_token`（vault）同时定义：`OTEL_EXPORTER_OTLP_ENDPOINT`、`OTEL_EXPORTER_OTLP_HEADERS=Authorization=Bearer%20<token>`（tonic 导出器自己读这个 env 并对值做 URL 解码）、`OTEL_RESOURCE_ATTRIBUTES=deployment.environment.name=<env>`。resource 属性经 SDK 的 env 检测器并入，落地为存储里的 `deployment.environment.name` 标签——所有面板与告警共用的唯一切片键。未守卫的环境渲染结果与之前逐字节一致，文件模式兜底不变。
- **路径** —— test 以 OTLP/gRPC 内网直连 collector（`:4317`）；prod 走一条专用 frp 隧道，把生产 VPS 上的回环端口（`127.0.0.1:4320`）映射到 NAS 上 collector 的 gRPC 口。隧道复用运维其他服务同款 frps，但作为独立的用户级 compose 项目（`frpc-wakewake`）存在，既不碰运维的主隧道客户端，也不需要 root。collector 对每个生产者做 bearer-token 鉴权；wakewake 有自己的按环境 token，以 `WW_OTLP_TOKEN_{TEST,PROD}` 加入 collector 的 token 列表。
- **补上缺失的记录点** —— `middleware/http_metrics.rs` 现在记录 `http_request_duration_seconds`（耗时、method、路由模板、status），以 `route_layer` 挂载使 `MatchedPath` 可用：`path` 标签是路由模板（`/api/v1/auth/login`）而非原始 URI，序列基数有界。它位于全局限流层之内；governor 的 429 由 `http_rate_limited_total` 承载，health 路由按设计不打点。
- **Grafana 工件进仓库** —— `devops/grafana/` 承载运行时面板 JSON（请求率/5xx 占比/p95、SSE 连接、命令、唤醒、邮件、限流、清理——按 `$env` 切片）、面板 provider（`wakewake.yml`）、托管告警规则（`rules-wakewake.yaml`：5xx 占比 warn/crit、p95 延迟、持续拒信；仅 prod，test 是红了属常态的快速迭代环境）。仓库是源；把它们装进运维 Grafana 的 provisioning 目录是文档化的两文件拷贝加重启，见 [devops/monitoring.md](../../../../devops/monitoring.zh.md)。查询语句是按第一次接线部署后在 VictoriaMetrics 里实测观测到的指标名与标签写的，不是凭假设。

## 验证

接线后的 test 环境：VictoriaMetrics 收到 `wakewake-server` 作用域、带 `deployment.environment.name=test` 的序列；一次 30 请求的突发产生完全对账的直方图计数（按路由模板 `10×422 + 20×429`）与 `http_rate_limited_total`；gRPC 路径被直接探测（无鉴权导出 → `Unauthenticated`，带 token 空导出 → 接受）。面板与告警文件在运维 Grafana 无错加载，告警规则进入调度器。

## 落选方案

**Prometheus 抓取端点（`/metrics`）。** 落选原因：要新 crate（`opentelemetry-prometheus` 或 `prometheus`）加一个暴露端口加一套带存储的抓取器——2 GB VPS 上新增活动部件与第二套指标词汇——而推送管道、collector、存储全部现成，后端也已链接导出器。

**永远文件模式（什么都不做）。** 落选原因：这种遥测要 SSH 上每台机器用肉眼看，没有跨环境切片、没有告警，且 logs/traces/metrics 与服务同盘——你最想要证据的故障模式，恰恰是把证据一起带走的那种。

**Grafana Cloud 或其他 SaaS。** 落选原因：运维观测栈已就位且已付费；SaaS 给个人工具添外部依赖、每宿主对第三方的出站流量和按用户计费。

**复用现有 HTTP 隧道而非新开 gRPC 隧道。** 落选原因：后端导出器是 tonic（gRPC）；复用仅 HTTP 的入口意味着改出厂源码里的导出协议并重验依赖矩阵，只为省一个回环端口。隧道是运维侧配置，不是应用代码。

**在 Grafana UI 里手搓面板（无仓库工件）。** 落选原因：UI 面板不可复现——Grafana 数据卷一丢，运维视图归零；下次环境接线从记忆开始。provisioned 文件像代码一样评审，一次拷贝即可重装。

## 后果

- 每个接线的环境在一个导出周期（60 s）内向共享栈发出 traces/metrics/logs；环境间仅以 `deployment.environment.name` 区分。
- prod 遥测依赖 frp 隧道存活：隧道故障退化为本地 JSONL 文件（双模式兜底不变），表现为序列停滞而非数据丢失。token 的轮换必须 collector 侧与 vault 侧同步进行，否则导出静默 401。
- 管道里没有常开心跳指标（仪表首次递增才出现），因此遥测断流告警不进 Grafana；可用性由 [可用性监控](2026-10-10-availability-monitoring.zh.md) 的 kuma push monitor 承担。
- HTTP 时长现在恰好有一个记录点；未来若出现第二个插桩位置，必须复用 `record_http_duration`，不得直接触碰直方图。
