# wakewake-agent

WakeWake agent：SSE 客户端 + RSA 密钥管理 + WoL（magic packet）+ Bemfa MQTT 集成。
运行在用户内网的目标机器上，接收 server 下发的唤醒命令并发送 magic packet。

## 快速开始

从 [releases](../../releases) 下载对应平台的二进制，或自行编译：

```bash
cd backend
cargo build --release -p wakewake-agent
# 产物：backend/target/release/wakewake-agent
```

## 配置

`server_url` 和 `pairing_code` 必填，三种方式任选其一（可混用，优先级高者覆盖低者）：

### 方式一：配置文件（推荐长期使用）

搜索路径（按顺序，后者覆盖前者）：

1. `$WAKEWAKE_HOME/config.toml`（默认 `~/.wakewake/config.toml`）
2. `./wakewake.toml`（当前工作目录）
3. `--config <路径>` 显式指定

创建配置文件（参考 [`config.example.toml`](./config.example.toml)）：

```toml
# ~/.wakewake/config.toml
server_url   = "https://wakewake.app"
pairing_code = "a1b2c3d4e5f60718"
```

然后直接运行（无需任何参数）：

```bash
./wakewake-agent
```

### 方式二：环境变量

```bash
WAKEWAKE_SERVER_URL=https://wakewake.app \
WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
./wakewake-agent
```

适合 Docker / systemd 场景。完整字段映射见 [`config.example.toml`](./config.example.toml)。

### 方式三：CLI 参数

```bash
./wakewake-agent --server https://wakewake.app --pairing-code a1b2c3d4e5f60718
```

`--help` 查看全部参数。

## 内网自托管（自签 TLS）

若 server 部署在内网并用 Caddy `tls internal`（自签证书），agent 需编译时启用
`danger-insecure-tls` feature 来信任自签证书：

```bash
cargo build --release -p wakewake-agent --features danger-insecure-tls
./wakewake-agent --server https://192.168.5.200:8449 --pairing-code <code>
```

> ⚠️ `danger-insecure-tls` 会让 agent 信任**任何**证书（`danger_accept_invalid_certs(true)`）。
> 仅适用于受信任的内网环境。生产环境请用合法证书 + 默认编译（不带此 feature）。

## Docker

Docker 镜像通过环境变量注入配置，`WAKEWAKE_HOME=/data`（持久化 key.pem）：

```bash
docker run -d \
  -e WAKEWAKE_SERVER_URL=https://wakewake.app \
  -e WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
  -v agent_data:/data \
  ghcr.io/jukanntenn/wakewake-agent:latest
```

## 首次连接

agent 首次连接 server 时会：

1. 生成 RSA-2048 密钥对（持久化到 `$WAKEWAKE_HOME/key.pem`，权限 0600）
2. 通过 SSE 连接的 `X-Public-Key` 头上报公钥，完成配对
3. 之后接收 state/command 事件

pairing code 可在 server 的 agents 页轮换（旧码立即失效）。

## 配置字段

完整字段说明、环境变量映射、默认值见 [`config.example.toml`](./config.example.toml)。
