# WakeWake

[English](./README.md) | **简体中文**

一个自托管的 **Wake-on-LAN（网络唤醒）** 服务。在任意浏览器的任意地点，向家庭
网络中的设备发送 magic packet——无需把内网暴露到公网。

WakeWake 由 **Rust** 后端（axum + sqlx + tokio）+ 独立的 **WoL agent**（部署在
内网）+ **Next.js 16** 静态前端组成，打成单个多架构 Docker 镜像，前置 Caddy。
设计目标：在一台 **2 GB VPS** 上承载 **10 万并发 agent**。

> **当前状态：** 早期阶段，尚无正式 release——目前处于自托管「dogfooding」阶段。
> API 与配置可能调整。

---

## 工作原理

```
            ┌─────────────────────────────┐
            │   Cloudflare (DNS / CDN)    │  可选
            │   HTTPS → 源站              │
            └──────────────┬──────────────┘
                           │ 443 / h2
            ┌──────────────▼──────────────┐
            │   Caddy（TLS 终结 + 反向    │   2 GB VPS
            │   代理 + 静态前端托管）     │
            │   ┌─────────┐ ┌───────────┐ │
            │   │ 静态    │ │ Rust API  │ │   axum，HTTP/2 (h2c)
            │   │ 前端    │ │  + SSE Hub│ │   PostgreSQL 17
            │   └─────────┘ └─────┬─────┘ │
            └─────────────────────┼───────┘
                                  │ HTTPS（agent 主动外连）
            ┌─────────────────────▼───────┐
            │   wakewake-agent (Rust)     │   你的家庭内网
            │   • SSE 客户端（接收命令）  │
            │   • RSA 解密 MAC            │
            │   • 发送 UDP magic packet   │
            │   • 巴法云 MQTT（可选）     │
            └─────────────┬───────────────┘
                          │ UDP 255.255.255.255:9
                   ┌──────▼──────┐
                   │  NAS / PC   │
                   └─────────────┘
```

核心架构原则：

- **Agent 无状态**——磁盘上只有 `pairing_code`、`server_url` 与 RSA 密钥对；
  连接时由 server 全量推送状态。
- **Agent 只外连**——server 到 agent 方向无主动连接。
- **Server 永不知明文 MAC**——MAC 在浏览器用 RSA-OAEP + SHA-256 加密，
  只有 agent 持有私钥；server 只是密文中转站。
- **PostgreSQL 是唯一持久真相**；内存命令通道是易失的。

完整拓扑、组件职责与数据流图见 [`specs/architecture/`](./specs/architecture/)。

## 功能特性

- **网络唤醒**——任意浏览器发起，支持设备分组与唤醒历史
- **独立 Rust agent**——SSE 客户端 + RSA 密钥管理 + magic-packet WoL
- **MAC 端到端加密**（RSA-OAEP + SHA-256）——浏览器侧用 Web Crypto，
  server 是密文中转站，永不持有明文 MAC
- **巴法云 MQTT 集成**——把设备桥接到 [巴法云](https://bemfa.com) IoT 平台
  （device-sync-v3 生命周期，有界最终一致性）
- **认证**——JWT access + 轮换 refresh token、bcrypt、无状态 HMAC 密码重置、
  邮件端点的 PoW 防滥用
- **国际化**——next-intl，8 语言（en / zh / ja / ko / de / fr / es / pt）
- **多架构 Docker**——单镜像同时支持 `linux/amd64` + `linux/arm64`，
  s6 进程管理，Caddy 负责 TLS（ACME）与静态托管
- **为规模而生**——Rust async task（单连接约 1.2 KB），SSE 优先，
  设计目标 2 GB VPS 承载 10 万 agent

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

# 启动全栈（Postgres + 后端 + Caddy + 前端 dev server）
python3 devops/dev.py start

# 停止全部
python3 devops/dev.py stop
```

前端 dev server：`http://localhost:${FRONTEND_PORT:-3034}`。

### 手动 / 分组件运行

```bash
# 后端（在仓库根目录）
cd backend
cargo test --workspace          # 全部测试（需活的 PostgreSQL）
cargo test --lib                # 仅单元测试（无需 DB）
cargo build --release
cargo clippy --all-targets --all-features -- -D warnings

# 前端
cd frontend
pnpm install
pnpm dev                        # dev server
pnpm build                      # 静态导出 → out/
pnpm test:run                   # Vitest
pnpm lint && pnpm format:check

# E2E（自动起完整容器栈）
cd e2e && pnpm test
```

> **关于开发工具：** `devops/dev.py` 以 Docker 化后端做编排，方便起见。
> 你也可以直接 `cargo run` 跑后端、`pnpm dev` 跑前端。完整命令参考见
> [`AGENTS.md`](./AGENTS.md)。

## 部署

WakeWake 打成单个 all-in-one Docker 镜像（前端 + Rust 后端 + Caddy，由
s6-overlay 管理）+ 旁路 PostgreSQL。多架构（`linux/amd64`、`linux/arm64`）
镜像由 [`docker/build.py`](./docker/build.py) 构建。

```bash
# 1. 配置环境
cp docker/.env.example docker/.env
#   → 修改 JWT_SIGNING_KEY、REFRESH_SIGNING_KEY、PASSWORD_RESET__SECRET
#     （openssl rand -base64 32）和 POSTGRES_PASSWORD

# 2. 起服务
docker compose -f docker/docker-compose.yml up -d

# 3. 打开应用
#   https://<你的主机>:8443
```

迁移在 server 启动时自动执行（`sqlx::migrate!`），无需单独迁移步骤。首启会
幂等创建一个 bootstrap admin（见 `.env.example` → `WAKEWAKE_SECURITY__BOOTSTRAP_*`）。

### Agent

`wakewake-agent` 跑在内网的一台机器上。可本地编译运行，也可用已发布镜像——
完整指南（配置文件 / 环境变量 / CLI 参数 / 自签 TLS 模式）见
[`backend/crates/agent/README.md`](./backend/crates/agent/README.md)。

```bash
cd backend
cargo build --release -p wakewake-agent
./target/release/wakewake-agent \
  --server https://your-wakewake.example.com \
  --pairing-code <在 agents 页拿到的配对码>
```

## 项目结构

```
backend/
  crates/
    protocol/   协议共享类型（server + agent 共依赖）
    server/     axum HTTP server（后端二进制）
    agent/      独立 WoL agent（SSE + RSA + 巴法云 MQTT）
    seed/       压测 seed 二进制
  migrations/   sqlx 迁移（编译期嵌入，启动时自动应用）
frontend/       Next.js 16 应用（静态导出）
e2e/            Playwright（chromium；独立 workspace）
docker/         多架构 Dockerfile + build.py + compose
devops/         dev.py + docker-compose + Caddyfile + ansible/
specs/          设计规范（architecture / api / database / testing）
.github/        CI workflows（lint / test / build / 夜间 e2e）
```

## 测试

| 层 | 工具 |
|---|---|
| 后端单元 | `cargo test --lib` |
| 后端集成 | `cargo test --workspace`（真实 PostgreSQL，从不 mock） |
| 前端单元 | Vitest + Testing Library + jsdom |
| E2E | Playwright（chromium），夜间定时 + 推 main 时触发 |

测试金字塔：约 70% 单元 / 25% 集成 / 5% e2e。数据库、SSE Hub 与内部逻辑
从不 mock；外部 HTTP（巴法云、邮件）mock。完整策略见 [`specs/testing/`](./specs/testing/)。

## License

[GNU AGPL-3.0](./LICENSE)。© wakewake contributors。
