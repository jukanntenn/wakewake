# RFC: 数据库备份 —— 每日 pg_dump 入对象存储，带新鲜度检查与恢复演练

Status: implemented

[English](2026-10-10-database-backup.md) | 中文

## 问题

每个 wakewake 环境的全部状态——用户、设备、agent、集成、唤醒历史——都存在宿主机 `pgdata` 命名卷里的单个 Postgres 数据库中。部署的其他部分都不难重建：镜像在 registry 里，配置由仓库模板渲染，Caddy 无状态。数据库是唯一丢了就找不回来的资产，而它目前没有任何备份：宿主磁盘故障、一次坏迁移、一次误删 volume、或 VPS 上的勒索软件，每一种都意味着数据全损且无回滚路径。

存在但从未恢复过的备份只是假设，不是能力——所以本设计必须把验证变成例行公事，而不是事故现场的壮举。

## 决策

每个受监控环境（test、prod）每日做一次逻辑转储备份到环境专属的对象存储桶、每日自检新鲜度、每月演练可恢复性。全部以部署用户身份运行，宿主机零 root。

- **转储管道** —— `devops/ansible/files/pg-dump-backup.py` 把 compose postgres 容器里的 `pg_dump --no-owner` 经 `zstd -19` 流入 `rclone rcat`，每日落一个对象：环境桶里的 `dumps/wakewake-YYYYmmddTHHMM.sql.zst`。纯 SQL 转储是格式多样性对冲：任何 Postgres 17 都能恢复（与块级状态无关）、能在物理方案必然失效的基镜像/页级损坏中幸存、紧急时人眼可读。`rclone` 限速（`--bwlimit 2M`），瘦上线上转储永远不会挤占源站流量。
- **调度** —— linger 下的 systemd 用户单元（`wakewake-backup@.service` 模板 + 三个 timer）：转储 03:30、新鲜度检查 05:30、恢复演练每月 2 号 05:00（1 号的转储已有货可演）。`Persistent=true` 补跑停机期间错过的触发。日志进 journal。
- **新鲜度检查** —— `backup_check.py` 在最新离线转储超过 26 h（静默备份死亡：漏触发、磁盘满、对象存储密钥被吊销）或本地 postgres 不可达（后续每次转储都会失败）时报错，并把 verdict 推给 uptime-kuma 的 `db backup` push monitor——备份失败像其他故障一样触发告警，而不是藏在日志文件里。
- **恢复演练** —— `backup-drill.py` 拉取最新转储、恢复进一次性 `postgres:17-alpine` 容器、与线上库比对 `users`/`wakes` 行数（wakes 宽限 2000 ≈ 个人工具规模一天的写入量）、随后清理全部痕迹。断言是行数相等，不只是退出码。
- **凭据与守卫** —— `backup.env`（0600，由 `templates/backup.env.j2` 渲染）承载 vault 加密的对象存储密钥对；endpoint/region/桶名是非秘密的环境变量，test 与 prod 分桶，晋升链永不混桶。整层守卫在 vault 里是否定义 `b2_key_id`：运维建好桶、写入密钥之前，部署保持绿色且与备份层之前逐字节一致（setup-order 契约，激活手册见 [devops/backup.md](../../../../devops/backup.zh.md)）。删掉变量永远不会触发卸载。

## 验证

在 test 环境：转储管道在桶里存入一个 zstd 压缩对象且 `rclone lsl` 可列出；`backup_check.py` 报告新鲜度并向 kuma monitor 推送 `up`；`backup-drill.py` 把最新转储恢复进一次性容器并比对线上行数一致。playbook 在该层关闭与打开两种渲染下都不触碰无关任务。

## 落选方案

**WAL 归档（持续 PITR）。** 落选原因：对这个数据集——几张表、写入率趋零的个人唤醒工具——24 h RPO 值得几分钟的搭建成本，而 WAL 归档买到没人需要的亚分钟 RPO，代价却是实打实的：要一个烤入 pgBackRest 的派生 postgres 镜像（原生 `postgres:17-alpine` 既不带 pgbackrest 也没有部署用户可用的构建路径）、compose command 行里耦合 `archive_command`、2 GB VPS 上的 spool/队列磁盘管理，以及一条太复杂、没法 casually 验证的恢复路径（基础备份 + WAL 回放）。若 RPO 要求收紧，下面的每日转储层仍作为损坏对冲保留，PITR 可以叠加其上。

**仅本地转储（宿主磁盘）。** 落选原因：备份数据库的同一块磁盘装着备份——磁盘故障、整机入侵、误删一次性带走两份副本。离线才是全部意义；对象存储还免费给了版本化与生命周期保留。

**托管快照（云厂商）。** 落选原因：环境横跨阿里云 VPS 与家庭内网机器，没有统一的厂商面；快照是块级的（继承页级损坏）；恢复需要部署路径刻意避开的厂商控制台权限。

**宿主机上的 cron + supervisor。** 落选原因：两者在这些宿主上安装都需要 root，而部署 playbook 的契约是部署用户不需要免密 sudo。带 linger 的 systemd 用户单元给出同样的常驻语义（timer 按点触发、`Restart=always` 覆盖心跳类任务），全程用户态。

## 后果

- 每环境每日一个 zstd 压缩的纯 SQL 对象落到离线；保留期是桶生命周期策略（转储默认 14 天过期），见 [devops/backup.md](../../../../devops/backup.zh.md)。
- 备份健康与服务本身同权重监控：备份死了 kuma monitor 一天内变红，恢复路径坏了月度演练变红，两者都无法静默失败。
- 缺少 rclone 包的宿主上，部署用户 home 下会多一个 `~/.local/bin` 静态二进制——由 `group_vars/all.yml` 的 `rclone_version` pin 住，零 root。
- RPO 是 24 h：全损场景下最多丢一天的唤醒历史与账号变更。恢复手册（停 postgres → 回放转储 → 验证 → 立即做一次全新全量）在 [devops/backup.md](../../../../devops/backup.zh.md)；演练宽限量按当前个人工具写入率调校，写入量上台阶后需重估。
