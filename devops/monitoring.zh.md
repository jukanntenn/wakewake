# 监控与可观测（test / prod）

[English](monitoring.md) | 中文

三套系统各管一个信号：**uptime-kuma** 管可用性，**Beszel** 管宿主/容器资源，**Grafana**（架在共享 OTLP 管道上）管服务质量。决策与落选方案见 [OTLP 接线](../.agents/rfcs/implemented/process/2026-10-10-otlp-observability-wiring.zh.md)、[可用性](../.agents/rfcs/implemented/process/2026-10-10-availability-monitoring.zh.md)、[Beszel](../.agents/rfcs/implemented/process/2026-10-10-host-metrics-beszel.zh.md) 三份 RFC。

## 拓扑

```
wakewake@fn (test)     ──OTLP/gRPC LAN直连──►  otelcol @ NAS ──► VictoriaMetrics / VictoriaLogs / Jaeger ◄── Grafana @ NAS
wakewake@abj (prod)    ──OTLP/gRPC 127.0.0.1:4320 ──frp隧道──►  otelcol gRPC :4317 @ NAS
heartbeat/backup push  ──test: LAN直连 / prod: 127.0.0.1:3002──► uptime-kuma @ armbian
beszel agents          ──test: LAN直连 / prod: 127.0.0.1:8091──► beszel hub @ NAS
```

观测栈（otelcol、各存储、Grafana、Beszel hub、uptime-kuma）归运维。仓库持有：下文的部署接线、心跳/备份检查生产者、Beszel agent compose、`devops/grafana/` 的 Grafana 工件。这些服务的公网边缘就在生产 VPS 上（`*.bytehome.fun` → 边缘 caddy → 回环 frp 端口 → 家里）；该 VPS 上的 wakewake 生产者直接用回环端口，刻意绕开公网边缘。

## OTLP 接线（部署侧）

`group_vars/<env>/env.yml` 设 `otel_otlp_endpoint`（test：`http://192.168.5.57:4317`；prod：`http://127.0.0.1:4320`）；vault 存 `otel_otlp_token`。compose 模板仅在两者齐备时渲染 `OTEL_EXPORTER_OTLP_ENDPOINT`、`OTEL_EXPORTER_OTLP_HEADERS`（bearer）、`OTEL_RESOURCE_ATTRIBUTES=deployment.environment.name=<env>`。VictoriaMetrics 里的指标/标签形态：仪表名保持原样（`http_request_duration_seconds` → `_bucket/_sum/_count`、`sse_connections_active`、计数器带 `_total`），resource 属性落成标签（`deployment.environment.name`、`service.name=wakewake-server`、`scope.name`）。

token 轮换是双侧的：在 NAS 的 collector env 里增/换 `WW_OTLP_TOKEN_{TEST,PROD}`、在 wakewake vault 里同步，重启 collector，重部署。

## Grafana

仓库工件：`devops/grafana/wakewake.yml`（面板 provider）、`devops/grafana/json/wakewake-runtime.json`（运行时面板，`$env` 变量按 `deployment.environment.name` 切片）、`devops/grafana/rules-wakewake.yaml`（托管告警规则）。

装到/更新到运维 Grafana（NAS，`~/docker/grafana`）：

```bash
scp devops/grafana/wakewake.yml           <nas>:docker/grafana/provisioning/dashboards/
scp devops/grafana/json/wakewake-runtime.json <nas>:docker/grafana/provisioning/dashboards/json/wakewake/
scp devops/grafana/rules-wakewake.yaml    <nas>:docker/grafana/provisioning/alerting/
ssh <nas> 'cd docker/grafana && docker compose restart grafana'
```

告警规则（文件夹 `wakewake`，仅 prod）：HTTP 5xx 占比 > 5 %（5 分钟，warning）与 > 20 %（2 分钟，critical）；p95 延迟 > 1 s（10 分钟，warning）；外发邮件连续 15 分钟被拒（warning——总闸或预算耗尽意味着注册/重置邮件发不出去）。通知路由沿用运维 Grafana 既有的联系人点与策略。

## uptime-kuma 监控项

在 kuma UI（运维实例）手工创建。命名：`wakewake · <env> · <对象> (<层>)`；每环境一个分组；通用设置：心跳间隔 60 s、重试 3 次、重试间隔 60 s；push 型：心跳重试 2 次 / 备份 1 次。

| 监控项 | 类型 | 目标 | 备注 |
|---|---|---|---|
| wakewake · prod · homepage (edge) | HTTP(s) | `https://wakewake.online/` | 期望 200；开启证书 + 域名到期通知 |
| wakewake · prod · origin health | HTTP(s) JSON 查询 | `https://wakewake.online/api/v1/health` | `$.status == ok` |
| wakewake · prod · host heartbeat (push) | Push，120 s | push URL → vault `kuma_heartbeat_url`（prod） | 生产者：`heartbeat.py` |
| wakewake · prod · db backup (push) | Push，86400 s | push URL → vault `kuma_backup_url`（prod） | 生产者：`backup_check.py` |
| wakewake · test · homepage | HTTP(s) | `https://192.168.5.200:8449/` | 忽略 TLS（自签） |
| wakewake · test · origin health | HTTP(s) JSON 查询 | `https://192.168.5.200:8449/api/v1/health` | 忽略 TLS |
| wakewake · test · host heartbeat (push) | Push，120 s | push URL → vault（test） | |
| wakewake · test · db backup (push) | Push，86400 s | push URL → vault（test） | |

push URL 契约：生产者需要**裸**端点（`…/api/push/<token>`，不带查询串），自行追加 `?status=…&msg=…`——URL 若已带 `?status=`，kuma 会按数组解析、把每次推送都判 down。

### 心跳 setup order（每环境一次）

1. 在 kuma 建 host-heartbeat push monitor；复制**裸** push URL。
2. 写 vault：`ansible-vault encrypt_string --vault-id wakewake-<env>@~/.local/bin/avpm-client --encrypt-vault-id wakewake-<env> '<url>' --name kuma_heartbeat_url >> devops/ansible/group_vars/<env>/vault.yml`
3. 部署：`ansible-playbook devops/ansible/deploy.yml -l <env>`——受守卫的层级安装 `heartbeat.py`、单元、开 linger 并开始推送。
4. 验证：`systemctl --user status wakewake-heartbeat`，kuma monitor 变绿且 `msg` 形如 `health + postgres ready`。

移除（如需）：删 vault 变量、删单元（`systemctl --user disable --now wakewake-heartbeat && rm ~/.config/systemd/user/wakewake-heartbeat.service`）、删 `{{ app_path }}/heartbeat.py`、删 kuma monitor。

## Beszel agents

每 hub 一次（运维实例，仅 WebSocket 模式）：在 hub 的 **Settings → Tokens & Fingerprints** 启用**永久通用 token**（或以登录用户身份 `GET /api/beszel/universal-token?enable=1&permanent=1`），并从 add-system 对话框复制**公钥**。0.20+ 没有 per-system token——agent 出示通用 token，hub 按 agent 指纹自动注册系统。然后，每环境一次：

1. `group_vars/<env>/env.yml`：`beszel_hub_url` 已设好（test：`http://192.168.5.57:8090`；prod：`http://127.0.0.1:8091`）；补 `beszel_agent_key: "ssh-ed25519 AAAA…"`（公开，非秘密）。
2. 通用 token 写 vault：`… --name beszel_agent_token >> devops/ansible/group_vars/<env>/vault.yml`
3. 部署。受守卫的层级渲染 `{{ beszel_agent_path }}/docker-compose.yml`、拉镜像、起 agent。
4. 首次连接会以 agent 宿主名自动注册系统——在 hub 里改名为 `wakewake-test` / `wakewake-prod`。验证：数秒内系统上线并带宿主 + 容器指标。

轮换通用 token 是 hub 级操作（绑定 hub 用户，每用户一把永久 token）：铸新 token、两环境 vault 同步、重部署。

hub 上生效的阈值：磁盘 90 %、内存 92 %、CPU 95 %（各持续 2 分钟）、agent 掉线 1 分钟。hub 侧设置（用户、通知、SQLite 数据目录的备份）归运维。

## 告警分诊

| 红的信号 | 可能的故障 | 第一步 |
|---|---|---|
| 边缘首页 down，其余绿 | 边缘/DNS/CF | 查边缘 caddy + DNS，再从外部 `curl` |
| origin health down，心跳绿 | 应用层（网关路由、后端） | `docker logs wakewake`、宿主回环 `/api/v1/health` |
| 心跳 push down | 进程、DB 或宿主死亡 | `systemctl --user status wakewake-heartbeat`、`docker compose ps`、`journalctl --user` |
| db backup push down | 备份管道 | 见 [backup.md](backup.zh.md) 故障模式 |
| Grafana 5xx/p95 规则 | 服务质量 | 运行时面板，再按 `deployment.environment.name` 查 VictoriaMetrics |
| Beszel 磁盘/内存/CPU | 宿主资源 | hub 系统页看逐容器分解 |
