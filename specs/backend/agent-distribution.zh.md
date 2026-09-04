# Agent 分发（一键安装）

[English](agent-distribution.md) | 中文

面向小白用户的 agent 获取、配置与常驻运行路径。设计决定（终端态）：双通道分发（GitHub Releases 二进制 + Docker Hub 生产镜像）、install.sh 一行安装、`service install` 子命令常驻、`tls.ca_cert` 覆盖自签场景。实现位于仓库根 `install.sh`、`backend/crates/agent/src/service.rs`、`.github/workflows/release.yml`、`docker/agent-prod.Dockerfile`、前端 `lib/agent-command.ts`。

## 渠道与资产矩阵

- **二进制**：GitHub Releases（tag `v*` 触发 `release.yml`，taiki-e/upload-rust-binary-action），三目标 musl **静态**链接（老 NAS 固件 glibc 免疫）：
  - `x86_64-unknown-linux-musl`（x86 NAS / 小主机）
  - `aarch64-unknown-linux-musl`（Pi 3/4/5、ARM NAS）
  - `armv7-unknown-linux-musleabihf`（老 Pi / 32 位 ARM NAS；armv6 不支持）
- **资产命名** `$bin-$target.tar.gz`（**不含版本号**，taiki-e `archive: $bin-$target`）→ `releases/latest/download/<asset>` 是跨版本稳定直链，install.sh 无需 GitHub API 解析 tag。每目标附 `<archive>.sha256`（标准 `sha256sum` 格式）。
- **Docker 镜像**：`jukanntenn/wakewake-agent`（Docker Hub，与主镜像同仓库模式）。生产镜像 = `docker/agent-prod.Dockerfile`（**单进程 entrypoint**，无 s6/sniffer）——与 E2E 专用 `docker/agent.Dockerfile`（danger feature + 双进程）是两个构建，互不复用。发布走 `docker-publish.yml` 的 agent job（同 tag 触发、原生 arm runner + imagetools 合并）。
- **Windows / macOS**：planned，README 标注。WoL 场景中 Windows 机是被唤醒目标而非 agent 宿主；常开宿主天然是 Linux NAS/Pi/小主机。
- **版本兼容承诺**：install.sh 永远拉 latest；`protocol` crate 字段只加不删（state snapshot 已有 version 水位），agent 与 server 版本解耦。

## install.sh 契约（仓库根，POSIX sh）

UI 生成的一行命令（`sh -s --` 传参，避免引号地狱）：

```sh
curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh | sh -s -- --server <origin> --code <配对码>
```

脚本步骤（失败即停，`set -eu`）：

1. 参数解析 `--server` / `--code`（支持 `--x=y` 与 `--x y` 两种形式），缺参报用法退出。
2. 平台检测：`uname -s` 必须 Linux；`uname -m` 映射 `x86_64|amd64`→x86_64 目标、`aarch64|arm64`→aarch64、`armv7l|armv8l`→armv7，`armv6l`/其他报「暂不支持」并列出支持矩阵。
3. 下载 `releases/latest/download/wakewake-agent-<target>.tar.gz` + `.sha256`；**逐字节比较哈希**（不依赖 `sha256sum -c`，busybox/玩具箱兼容）。
4. 解压安装：root（`/usr/local/bin` 可写）装 `/usr/local/bin/wakewake-agent`；否则装 `~/.local/bin` 并提示 PATH。
5. 写配置：`$WAKEWAKE_HOME`（默认 `~/.wakewake`）下 `config.toml`——**已存在则先备份为 `config.toml.bak.<时间戳>`**（保留 key.pem 与用户自定义字段无从合并，备份是唯一安全策略），写入最小内容 `server_url` + `pairing_code`，`chmod 600`（文件含配对码）。`key.pem` 永不触碰。
6. **前台 exec 启动**（`exec $BIN --config <cfg>`）：小白在终端立刻看到连接日志、agents 页变绿；启动前打印常驻提示（`sudo wakewake-agent service install`）。

幂等性：重复执行重新下载/覆盖二进制、备份旧配置后重写——配对码轮换后重跑同一行命令即完成重配。

## 常驻运行：`service install` 子命令

前台跑的必然结果：ssh 断开 agent 即死。常驻 = `sudo wakewake-agent service install`（cloudflared 同款模式）：

- **仅 Linux systemd**：非 Linux 报错并指引 Docker 路径；`systemctl` 不存在同样报错。
- **root 必须**：解析 `/proc/self/status` 的 `Uid:` 首字段判断 euid；非 root 报「需要 sudo」。
- **用户语义**：unit 以 `User=<SUDO_USER 或当前用户>` 运行（`sudo` 下服务仍以原用户身份跑，文件权限与前台运行一致）；HOME 经 `getent passwd` 解析为该用户的真实家目录（回退 `/home/<user>`），unit 里显式 `Environment=HOME=`——config 搜索与 `home_dir`（key.pem/logs）与前台行为完全一致。
- **配置定位**：`--config` 显式传 `<WAKEWAKE_HOME>/config.toml`（或 CLI `--config`）；文件不存在时报错并引导先跑 install.sh / 建配置（绝不静默用错误路径起服务）。
- **unit 内容**（模板在 `service.rs`，可测试）：`After/Wants=network-online.target`、`Restart=on-failure` + `RestartSec=5`、`ExecStart=<current_exe> --config <cfg>`。路径含空格时 systemd 引号转义。写 `/etc/systemd/system/wakewake-agent.service`（0644）→ `daemon-reload` → `enable --now`。
- Docker 路径用户**不需要**也**不应该**用它：容器内无 systemd，`--restart unless-stopped` 已覆盖。

UI 配套：`/agents` 高级折叠区新增「开机自启」块（命令 + 「Docker 安装无需此步」说明），8 locale。

## 自签 TLS：`tls.ca_cert`

矛盾根源：`danger-insecure-tls` 是编译期 feature（e2e 专用，CI 发布校验生产二进制绝不携带），预编译二进制无法信任 Caddy `tls internal` 自签证书。解法是**信任自己的 CA，而非关闭校验**：

- agent 新增 `[tls] ca_cert = "<PEM 路径>"` 配置（env `WAKEWAKE_TLS__CA_CERT`），`http_client::build_http_client` 读 PEM 后 `reqwest::Certificate::from_pem` + `add_root_certificate`。默认 `None` 用系统信任库，行为不变。
- 内网自托管用户一次性导出 Caddy 内部 CA root（`/data/caddy/pki/authorities/local/root.crt`）到 agent 机器并在 config.toml 指向它。
- `danger-insecure-tls` 编译期 feature 原样保留（继续 e2e 专用）；生产二进制**永远不存在**「信任任何证书」的开关——防社工边界不后撤。
- `build_http_client` 签名返回 `anyhow::Result`（CA 文件读取失败报可读错误；PEM 内容合法性由 reqwest/rustls 在握手期最终校验）。

## Docker 路径：`--network host` 是硬前提

- **为什么**：magic packet 目标是受限广播 `255.255.255.255:9`，路由器/网桥从不转发（RFC 919）。bridge 网络下广播只泛洪到 `172.17.0.0/16`，物理网卡上没有包；且 `send_to` 返回 Ok、agent 上报 `success:true`——**静默失败**（`wol.rs` 已为此记录 `source_ip` 排障）。
- **host 网络下为什么成立**：容器共用宿主网络栈，`0.0.0.0` + `SO_BROADCAST` 的受限广播从默认路由网卡（LAN 物理网卡）发出。agent 纯外连、无监听端口，host 模式无安全损失。
- UI 与 README 的 `docker run` 命令**必须**含 `--network host` + `--restart unless-stopped` + `-v wakewake-agent-data:/data`。
- Docker Desktop（Mac/Win）host 网络不完整——agent 宿主场景（NAS/Pi/Linux）不涉及，README 不展开。

## UI（agents 页终端卡）

- `lib/agent-command.ts` 两个生成器：`buildAgentInstallCommand`（curl 一行，主 tab）与 `buildAgentDockerCommand`（docker run 一行）；**单条命令单行**（终端卡横向滚动，`$` 前缀按行渲染，续行符会破坏视觉）。`--server` 取 `window.location.origin`，配对码由调用方传入（沿用模板槽机制，页面结构不动，仅终端卡头部加 tab 切换）。
- tab：`Linux`（默认）/ `Docker`；复制按钮复制当前 tab 命令。pending 引导态与 offline 修复态共用。

## 验收

- `service.rs`：unit 模板渲染（字段、路径含空格的 systemd 引号）、euid 解析、配置缺失错误路径有测试；`cargo test -p wakewake-agent` 全绿。
- `install.sh`：`sh -n` 语法检查 + shellcheck clean；哈希不匹配/平台不支持/缺参数路径有明确报错文案。
- CI：`release.yml` 产三目标 tar.gz + .sha256；`docker-publish.yml` agent job 产 `jukanntenn/wakewake-agent` 多架构镜像；两工作流均不携带 `danger-insecure-tls`。
- 前端：`agent-command.test.ts` 覆盖两个生成器（origin/配对码内嵌、`--network host` 存在）；新 i18n 键 8 locale 全量（en/zh/ja/ko/de/fr/es/pt）；`pnpm lint` + `test:run` + `format:check` 绿。
- 文档：根 README 对（`README.md` / `README.zh.md`）agent 节、agent README（下载/docker/service/ca_cert/平台矩阵）与实际命令一字不差——不虚构未存在的渠道。
