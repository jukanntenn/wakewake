# WakeWake

[English](README.md) | 简体中文

一个自托管的 **Wake-on-LAN（网络唤醒）** 服务。在任意浏览器的任意地点，向家庭网络中的设备发送 magic packet——无需把内网暴露到公网。

WakeWake 由 **Rust** 后端（axum + sqlx + tokio）+ 独立的 **WoL agent**（部署在内网）+ **Next.js 16** 静态前端组成，打成单个多架构 Docker 镜像，前置 Caddy。设计目标：在一台 **2 GB VPS** 上承载 **10 万并发 agent**。

> **当前状态：** 早期阶段，尚无正式 release——目前处于自托管「dogfooding」阶段。API 与配置可能调整。

---

## 工作原理

```
            ┌─────────────────────────────┐
            │   Cloudflare (DNS / CDN)    │  optional
            │   HTTPS → origin            │
            └──────────────┬──────────────┘
                           │ 443 / h2
            ┌──────────────▼──────────────┐
            │   Caddy  (TLS + reverse     │   2 GB VPS
            │   proxy + static frontend)  │
            │   ┌─────────┐ ┌───────────┐ │
            │   │ static  │ │ Rust API  │ │   axum, HTTP/2 (h2c)
            │   │ frontend│ │  + SSE Hub│ │   PostgreSQL 17
            │   └─────────┘ └─────┬─────┘ │
            └─────────────────────┼───────┘
                                  │ HTTPS (agent dials out)
            ┌─────────────────────▼───────┐
            │   wakewake-agent (Rust)     │   your home LAN
            │   • SSE client (commands)   │
            │   • RSA-decrypts the MAC    │
            │   • sends UDP magic packet  │
            │   • Bemfa MQTT (optional)   │
            └─────────────┬───────────────┘
                          │ UDP 255.255.255.255:9
                   ┌──────▼──────┐
                   │  NAS / PC   │
                   └─────────────┘
```

核心架构原则：

- **Agent 无状态**——磁盘上只有 `pairing_code`、`server_url` 与 RSA 密钥对；连接时由 server 全量推送状态。
- **Agent 只外连**——server 到 agent 方向无主动连接。
- **Server 永不知明文 MAC**——MAC 在浏览器用 RSA-OAEP + SHA-256 加密，只有 agent 持有私钥；server 只是密文中转站。
- **PostgreSQL 是唯一持久真相**；内存命令通道是易失的。

设计规范索引见 [`specs/`](specs/README.zh.md)。

## 功能特性

- **网络唤醒**——任意浏览器发起，支持设备分组与唤醒历史
- **独立 Rust agent**——SSE 客户端 + RSA 密钥管理 + magic-packet WoL
- **MAC 端到端加密**（RSA-OAEP + SHA-256）——浏览器侧用 Web Crypto，server 是密文中转站，永不持有明文 MAC
- **巴法云 MQTT 集成**——把设备桥接到[巴法云](https://bemfa.com) IoT 平台（device-sync-v3 生命周期，有界最终一致性）
- **认证**——JWT access + 轮换 refresh token、bcrypt、无状态 HMAC 密码重置、邮件端点的 PoW 防滥用
- **国际化**——next-intl，8 语言（en / zh / ja / ko / de / fr / es / pt）
- **多架构 Docker**——单镜像同时支持 `linux/amd64` + `linux/arm64`，s6 进程管理，Caddy 负责 TLS（ACME）与静态托管
- **为规模而生**——Rust async task（单连接约 1.2 KB），SSE 优先，设计目标 2 GB VPS 承载 10 万 agent

## 技术栈

| 层 | 技术 | 版本 |
|---|---|---|
| 后端 | Rust、axum、sqlx、tokio | edition 2021，MSRV 1.95 |
| Web | axum 0.8（HTTP/2 h2c）、hyper 1、tower-http 0.6 | |
| 数据库 | PostgreSQL | 17 |
| 认证 | jsonwebtoken 10、bcrypt 0.19（cost=10）、RSA-OAEP+SHA-256 | |
| Agent | Rust、reqwest 0.12（SSE）、rumqttc 0.24（巴法云 MQTT） | |
| 前端 | Next.js 16（`output: "export"`）、React 19、TypeScript | |
| 样式 / UI | Tailwind CSS 4、@base-ui/react、Zustand、TanStack Query | |
| 反向代理 | Caddy 2（TLS / ACME / h2c / 静态文件） | |
| 容器 | Docker + docker buildx、s6-overlay（多进程镜像） | |

## 快速开始（本地开发）

前置依赖：Docker、Python 3、Node.js 22 + pnpm 11、Rust 1.95。

```bash
git clone https://github.com/jukanntenn/wakewake.git
cd wakewake

# Start the full stack (Postgres + backend + Caddy + frontend dev server)
python3 devops/dev.py start

# Stop everything
python3 devops/dev.py stop
```

前端 dev server：`http://localhost:${FRONTEND_PORT:-3034}`。

### 手动 / 分组件运行

```bash
# Backend (from repo root)
cd backend
cargo test --workspace          # all tests (needs a live PostgreSQL)
cargo test --lib                # unit tests only (no DB)
cargo build --release
cargo clippy --all-targets --all-features -- -D warnings

# Frontend
cd frontend
pnpm install
pnpm dev                        # dev server
pnpm build                      # static export → out/
pnpm test:run                   # Vitest
pnpm lint && pnpm format:check

# E2E (spins up the full container stack)
cd e2e && pnpm test
```

> **关于开发工具：** `devops/dev.py` 起基础设施（Docker 内 postgres + mailpit），后端（`cargo watch`）与前端（`pnpm dev`）以宿主进程热重载运行。完整命令参考见 [`AGENTS.md`](AGENTS.md)。

## 部署

WakeWake 打成单个 all-in-one Docker 镜像（前端 + Rust 后端 + Caddy，由 s6-overlay 管理）+ 旁路 PostgreSQL。多架构（`linux/amd64`、`linux/arm64`）镜像由 [`docker/build.py`](docker/build.py) 构建。

```bash
# 1. Configure (app config + postgres credentials)
cp docker/config.example.toml docker/config.toml
cp docker/.env.example docker/.env
#   → edit the 3 secrets in config.toml (openssl rand -base64 32),
#     public_url, and POSTGRES_PASSWORD in .env

# 2. Bring up the stack
docker compose -f docker/docker-compose.yml up -d

# 3. Open the app
#   http://<your-host>:8443
```

迁移在 server 启动时自动执行（`sqlx::migrate!`），无需单独迁移步骤。首启会幂等创建一个 bootstrap admin（见 `backend/config.example.toml` → `WAKEWAKE_SECURITY__BOOTSTRAP_*`）。

### Agent

`wakewake-agent` 跑在内网的一台机器上（须与被唤醒设备同一路由器下）。在 server 的 **agents 页**复制一行安装命令（内嵌你的站点地址与配对码）：

```bash
curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh \
  | sh -s -- --server https://your-wakewake.example.com --code <pairing-code>
```

脚本下载 musl 静态二进制（x86_64 / aarch64 / armv7）→ sha256 校验 → 安装 → 写配置 → 前台启动。验证成功后常驻运行：

```bash
sudo wakewake-agent service install        # bare metal: systemd, starts on boot
```

或用 Docker（**必须 `--network host`**——WoL 广播不跨 bridge 网段）：

```bash
docker run -d --name wakewake-agent --network host --restart unless-stopped \
  -e WAKEWAKE_SERVER_URL=https://your-wakewake.example.com \
  -e WAKEWAKE_PAIRING_CODE=<pairing-code> \
  -v wakewake-agent-data:/data \
  jukanntenn/wakewake-agent:latest
```

完整指南（配置文件 / 环境变量 / CLI 参数 / 内网自签 TLS 的 `tls.ca_cert`）见 [`backend/crates/agent/README.zh.md`](backend/crates/agent/README.zh.md)。

## 项目结构

```
backend/
  crates/
    protocol/   shared wire types (server + agent)
    server/     axum HTTP server (the backend binary)
    agent/      standalone WoL agent (SSE + RSA + Bemfa MQTT)
    seed/       load-test seed binary
  migrations/   sqlx migrations (embedded, applied on startup)
frontend/       Next.js 16 app (static export)
e2e/            Playwright (chromium; separate workspace)
docker/         multi-arch Dockerfiles + build.py + compose
devops/         dev.py + dev-compose.yml (postgres + mailpit) + ansible/
specs/          design specs (indexed by specs/README.md)
.github/        CI workflows (lint / test / build / nightly e2e)
```

## 测试

| 层 | 工具 |
|---|---|
| 后端单元 | `cargo test --lib` |
| 后端集成 | `cargo test --workspace`（真实 PostgreSQL，从不 mock） |
| 前端单元 | Vitest + Testing Library + jsdom |
| E2E | Playwright（chromium），夜间定时 + 推 main 时触发 |

测试金字塔：约 70% 单元 / 25% 集成 / 5% e2e。数据库、SSE Hub 与内部逻辑从不 mock；外部 HTTP（巴法云、邮件）mock。已成文规范见 [`specs/`](specs/README.zh.md) 索引。

## License

[GNU AGPL-3.0](LICENSE)。© wakewake contributors。
