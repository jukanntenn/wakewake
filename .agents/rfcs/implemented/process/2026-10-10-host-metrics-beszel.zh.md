# RFC: 用 Beszel agent 做受监控宿主机的主机指标

Status: implemented

[English](2026-10-10-host-metrics-beszel.md) | 中文

## 问题

应用层之上的一切都是盲区：postgres 卷与容器日志把磁盘填满、小内存 VPS 在内存压力下换页换到死、镜像拉取期间的 CPU 饱和、容器在 `restart: always` 下重启循环而 health 端点毫不知情。应用遥测（OTLP）与可用性推送（kuma）都与宿主同生共死——没有谁能报告宿主本身的死亡。容器级可见性同样重要：应用与 postgres 兄弟容器是仅有的两个长驻容器，它们的资源退化最值得被看见。

## 决策

每个受监控宿主跑一个 Beszel agent；hub 归运维持在观测服务器上，仓库恰好只持有 agent 这一半。

- **agent 部署** —— `templates/beszel-agent-compose.yml.j2` 渲染一个单服务 compose 项目（`{{ beszel_agent_path }}`，应用的兄弟目录）：`network_mode: host`（直读宿主计数器；唯一流量是一条出站 WebSocket）、`/var/run/docker.sock` 只读挂载（容器指标；只读限制的是写不是看）、镜像 pin（`henrygd/beszel-agent:0.20.0`——0.19+ 的 agent 校验 hub 的 TLS 证书，pin 是下限）、`KEY` 设为 hub 公钥（非秘密——WebSocket 模式下用它校验 hub 的签名挑战）、`TOKEN` 设为 hub 的**通用注册 token**，vault 加密为 `beszel_agent_token`。部署任务要求 hub URL、key、token 三者齐备（凭据对原子：agent 从不半配出货）且永不卸载——移除是文档化的手工步骤。Beszel 0.20 起没有 per-system token：agent 出示通用 token，hub 首次连接时按 agent 指纹 find-or-create 系统记录——部署后一次性把自动注册的系统改名 `wakewake-<env>`。
- **hub 寻址** —— test 的 agent 内网直连 hub；prod 的 agent 经自己 VPS 上的回环端口（由运维边缘隧道回家，与 OTLP 入口、kuma 推送路径同款模式）连 hub。hub 以仅 WebSocket 模式运行（`DISABLE_SSH`）：agent 出站拨号，hub 从不回拨，任何受监控宿主都不暴露入站。
- **所有权边界** —— hub（其 SQLite 数据、用户、阈值、通知）归运维；仓库的手册 [devops/monitoring.md](../../../../devops/monitoring.zh.md) 记录 add-system 步骤、生效的告警阈值（磁盘 90 %、内存 92 %、CPU 95 %，持续 2 分钟；agent 掉线 1 分钟）与边界所在：Beszel 管*宿主与容器*资源告警，kuma 管*可用性*，Grafana 管*服务质量*——一个信号一个归属，不重复报警。

## 验证

两个环境上：agent compose 项目在 playbook 守卫任务下起来、容器 `running`，hub 在 agent 启动后数秒内显示两个系统 live 且带宿主与逐容器指标。vault 变量缺席时部署该层被跳过、playbook 保持绿色。

## 落选方案

**每台宿主上装 Netdata。** 落选原因：单机常驻 250–350 MB，而 agent 层的监控预算约 128–256 MiB；Beszel agent 是单个小 Go 二进制。Netdata 的深度（逐进程、eBPF）对"机器健康吗"是诊断力过剩。

**Prometheus 形态的栈（node_exporter + cadvisor + 抓取器）。** 落选原因：每台宿主多三个活动部件，外加某处的抓取器、存储与保留——把可观测接线已经裁决过的推拉之争再吵一遍，而在这些宿主上当时的答案是"不加新守护进程"。

**只靠 OTLP 承载主机指标。** 落选原因：主机指标会与应用遥测管道同命运（collector 挂 = 全盲），且 Rust 侧主机仪表生态相比一个能在应用混乱中持续上报的专用 agent 尚不成熟。

**扩展 uptime-kuma 的资源监控。** 落选原因：kuma 的资源监控（若有）是实例级的，同样同命运，且没有逐容器分解。

**每个项目各配一个 hub。** 落选原因：N 个 hub、N 套用户与通知，监控层存在的全部意义——独立于被监控系统——被稀释成各项目的雪花件。

## 后果

- 每个受监控 wakewake 宿主的主机与容器资源告警来自一个能在应用栈整体故障中幸存的系统。
- agent 读 Docker socket（只读）：它能看见共享宿主上的全部容器——单运维者机器上可接受，指向多租户宿主前值得再想一遍。
- agent 版本漂移是手动的：pin 在各环境变量里，升级 = 改 pin 重跑 playbook；hub 容忍同小版本内的 agent 混布。
- hub 丢失意味着丢告警*历史*（阈值与系统可重建——手册列了清单）；hub 的数据目录是运维的备份责任，已记入手册。
