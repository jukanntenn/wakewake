# RFC: 可用性监控 —— uptime-kuma 推送生产者与监控项清单

Status: implemented

[English](2026-10-10-availability-monitoring.md) | 中文

## 问题

部署好的 wakewake 环境至少有四种外部观察者在意的失败方式：公网边缘不可达（DNS、Cloudflare、宿主网关）、源站活着但在吐错误、进程或数据库卡死而边缘看起来完好、整个宿主机死透。这些此前无人看守：用户靠闹钟没响来发现服务挂了，就是当前的检测机制。

运维已在家用内网跑着一个 uptime-kuma 实例（内网与公网边缘都可达），所以这里的决策不是装哪个监控器，而是 wakewake 的这一半如何被持有与自动化，而不去伸手进运维的实例。

## 决策

kuma 实例归运维；仓库持有**生产者**（推送 verdict 的进程）与**清单**（待建监控项的权威列表），两半在 vault 加密的 push URL 上会合。

- **心跳生产者** —— `devops/ansible/files/heartbeat.py` 以 systemd 用户单元（`wakewake-heartbeat.service`，linger，`Restart=always`）跑在每个受监控宿主上：每 60 s 探测宿主健康 URL（`heartbeat_health_url` 按环境配置；body 必须是 `ok`；自签环境加 `--insecure`）**并**通过 `docker compose exec` 对应用使用的同一 socket 路径做 `pg_isready`——`/api/v1/health` 刻意只探活（Docker healthcheck 不因 DB 负载抖动），DB 探测由心跳自己补上。verdict 推给 kuma。
- **双失败通道** —— 每个 push monitor 的 down 要么来自显式的 `status=down` 推送（应用层失败，覆盖边缘盲区——push 路径绕过公网入口），要么来自沉默（宿主死透、脚本崩溃）。push URL 是秘密——持有它的人可以伪造 `up` 心跳掩盖事故——所以它只经单元文件（0600，vault 渲染）里的 `KUMA_HEARTBEAT_URL` 到位，绝不进脚本；备份层的 verdict 经 `backup.env` 里的 `KUMA_BACKUP_URL` 走同一契约。
- **裸 push URL 契约** —— 生产者在无查询串的 URL 后追加 `?status=…&msg=…`。多出的第二个 `?status=` 后缀（kuma 复制表单给的那个形态）会让 kuma 把 status 读成数组、每次推送都判 down；手册只保留裸 URL 形态。
- **按环境的推送路径** —— test 内网直连 kuma 实例；prod 推到自己 VPS 上的一个回环端口，由运维边缘隧道回家（与 OTLP 入口同款模式）。两个生产者都不经过公网边缘。
- **清单而非脚本** —— 监控项（边缘首页、源站 health JSON 查询、宿主心跳 push、db backup push；每环境一组，kuma 里按环境分组）在 [devops/monitoring.md](../../../../devops/monitoring.zh.md) 逐字段记载，一次性手工创建。告警送达（通知渠道、重试策略）是 kuma 侧配置，归运维。

## 验证

两个环境上：心跳单元在 linger 下运行，`systemctl --user status wakewake-heartbeat` 显示持续推送，kuma push monitor 报 up 且带实时 `msg` 载荷；停掉 postgres 容器后，心跳在一个间隔内翻转为 `status=down` 的 postgres 探测失败消息。setup-order 手册（建 monitor → vault 写 URL → 部署 → 验证）照写执行过一遍。

## 落选方案

**用 kuma 的 API 脚本化创建监控项。** 落选原因：为了恰好一次地配几个 monitor，引入一个对着无版本私有 API 的新依赖；这套 API 面比一次性搭建活得久，是持续的兼容性风险，而手册的字段表可以在 PR 里评审，改别人监控实例的代码不行。

**纯轮询监控（kuma 拨公网 URL，无生产者）。** 落选原因：它看到的是边缘不是源站——健康网关后面的卡死进程、健康进程后面的死数据库，看起来都是绿的。反向推送是唯一在网关与 DNS 全挂时仍然存活的观察位。

**用 Grafana 告警做可用性。** 落选原因：遥测管道没有常开序列（仪表首次递增才出现），基于缺失的告警天然抖动；可用性是推送契约，不是查询。

**更重的监控栈（Pingdom/UptimeRobot/Better Stack）。** 落选原因：运维实例已存在、自托管、通知接线已调好；SaaS 添加账号、配额，还让第三方看到每一个探测目标，换不来任何额外覆盖。

## 后果

- 可用性 verdict 从每个环境内部经绕过公网边缘的路径送达，因此"边缘全挂"与"宿主全挂"可区分（边缘红 + 心跳绿 vs 双红）。
- 心跳把健康信号拓宽到应用之外：postgres 死掉时 monitor 变 down，尽管 `/api/v1/health` 仍是 `ok`——这是刻意的，monitor 的 `msg` 会指名哪个探测失败。
- 轮换一个 push URL 意味着重建 kuma monitor、重写 vault、重部署——手册排好了这三步的顺序；URL 泄露给任何人是被伪造 `up` 的风险，所以 URL 与 token 同级保密。
