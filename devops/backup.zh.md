# 数据库备份（test / prod）

[English](backup.md) | 中文

每日逻辑转储备份到离线对象存储桶、备份静默死亡时会报警的新鲜度检查、证明转储可恢复的月度恢复演练。决策理由与落选方案见[备份 RFC](../.agents/rfcs/implemented/process/2026-10-10-database-backup.zh.md)。

## 什么在哪里跑

| 部件 | 位置 | 调度 |
|---|---|---|
| `pg-dump-backup.py` | 各宿主 `{{ app_path }}`，systemd 用户单元 `wakewake-backup@pg-dump-backup.py.service` | 每日 03:30 |
| `backup-check.py` | 同上 | 每日 05:30，verdict 推给 kuma |
| `backup-drill.py` | 同上 | 每月 2 号 05:00 |

三者都以部署用户运行（linger 已开；零 root）。凭据在 `{{ app_path }}/backup.env`（0600）：endpoint/region/桶来自 `group_vars/<env>/env.yml`，密钥对来自 vault（`b2_key_id` / `b2_secret_access_key`）。`zstd` 用宿主系统包；宿主缺 `rclone` 包时以静态二进制装进 `~/.local/bin`（版本由 `group_vars/all.yml` 的 `rclone_version` pin）。

日志：`journalctl --user -u wakewake-backup@pg-dump-backup.py.service`（其余两个单元同理）。

## 对象存储准备（一次性）

1. 建**私有、开版本化**的桶：`wakewake-backups`（prod）与 `wakewake-backups-test`（test）——分桶，晋升永不混装。
2. 建仅限该桶的应用密钥，权限只有 **writeFiles、readFiles、listFiles**——不能删。两个脚本只增不减。
3. 生命周期：`dumps/*` 的非当前版本 14 天后过期（当前对象由上表的每日转储备份节奏管理）。

## 首次激活（顺序有讲究）

整层守卫在 vault 里是否存在 `b2_key_id`——在那之前部署整体跳过、保持绿色。

1. 按上文建桶与密钥。
2. 把密钥对写入 vault（值不落任何明文文件）：
   ```bash
   ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client --encrypt-vault-id wakewake-test '<keyID>' --name b2_key_id >> devops/ansible/group_vars/test/vault.yml
   ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client --encrypt-vault-id wakewake-test '<appKey>' --name b2_secret_access_key >> devops/ansible/group_vars/test/vault.yml
   ```
   （same for `wakewake-prod` and the prod vault; when the kuma backup monitor exists, add `kuma_backup_url` the same way）
3. 部署：`ansible-playbook devops/ansible/deploy.yml -l test`——脚本、`backup.env`、单元与 timer 落地并武装。
4. 手动跑第一次转储并确认落桶：
   ```bash
   systemctl --user start wakewake-backup@pg-dump-backup.py.service
   journalctl --user -u wakewake-backup@pg-dump-backup.py.service -n 5
   ~/.local/bin/rclone lsl :s3,provider=Other,endpoint=<endpoint>:<bucket>/dumps/  # needs the env vars from backup.env
   ```
5. 检查与演练同理：
   ```bash
   systemctl --user start wakewake-backup@backup-check.py.service
   systemctl --user start wakewake-backup@backup-drill.py.service   # prints live vs restored row counts
   ```

## 灾难恢复

宿主丢失后在全新宿主重建（或数据库原地损坏）时：

1. 照常准备/部署环境（`ansible-playbook devops/ansible/deploy.yml -l prod -K`），但在应用承接流量**之前**停下；随后停 postgres：
   ```bash
   cd ~/docker/wakewake && docker compose stop wakewake
   ```
2. 取最新转储，恢复进全新的 postgres 容器：
   ```bash
   source ./backup.env   # or parse it; then:
   ~/.local/bin/rclone cat ":s3,provider=Other,endpoint=$B2_S3_ENDPOINT:$B2_BUCKET/dumps/<newest>.sql.zst" \
     --s3-access-key-id "$B2_ACCESS_KEY_ID" --s3-secret-access-key "$B2_SECRET_ACCESS_KEY" \
     --s3-region "$B2_S3_REGION" --s3-no-check-bucket \
     | zstd -dc | docker compose exec -T postgres psql -U wakewake -d wakewake
   ```
   （原地损坏恢复时目标库非空——先 drop 再 recreate。）
3. 拉起应用并验证：`docker compose up -d`，然后 `curl /api/v1/health` 并登录。
4. **立即做一次全新全量转储**（`systemctl --user start wakewake-backup@pg-dump-backup.py.service`）：这份 DR 副本此刻是唯一副本。

超出最后一次转储的时间点不可恢复——RPO 按设计是 24 h（[RFC](../.agents/rfcs/implemented/process/2026-10-10-database-backup.zh.md) 记录了取舍）。

## 故障模式

| 症状 | 先查什么 |
|---|---|
| kuma `db backup` monitor 红 | `journalctl --user -u wakewake-backup@backup-check.py.service`——`msg` 指名失败的探测 |
| 新鲜度探测失败 | 转储 timer 是否触发？`systemctl --user list-timers`；磁盘满？对象存储密钥过期？ |
| 转储管道退出码 1 | journal 指明哪一段失败（pg_dump / zstd / rclone）；网络还是凭据 |
| 演练行数对不上 | 看转储年龄——忙日超过一天的写入意味着宽限量太紧；调 `WAKES_SLACK` |
