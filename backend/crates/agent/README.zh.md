# wakewake-agent

[English](README.md) | 中文

WakeWake agent：SSE 客户端 + RSA 密钥管理 + WoL（magic packet）+ Bemfa MQTT 集成。运行在用户内网的目标机器上，接收 server 下发的唤醒命令并发送 magic packet。

## 安装

### 一行安装（推荐）

server 的 agents 页生成内嵌配对码的一行命令（`--server` 取你的站点地址）：

```bash
curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh \
  | sh -s -- --server https://your-wakewake.example.com --code a1b2c3d4e5f60718
```

脚本自动完成：平台检测 → 下载 Release 二进制（musl 静态）→ sha256 校验 → 安装 → 写 `~/.wakewake/config.toml` → **前台启动**（页面变绿即配对成功）。幂等，配对码轮换后重跑同一行命令即可。

支持平台：`x86_64` / `aarch64` / `armv7`（Linux musl 静态，老 NAS 固件 glibc 免疫）。Windows / macOS 暂未支持（planned）。

### 手动下载

从 [releases](https://github.com/jukanntenn/wakewake/releases) 下载 `wakewake-agent-<target>.tar.gz`，校验 `.sha256` 后解压安装；或自行编译：

```bash
cd backend
cargo build --release -p wakewake-agent
# 产物：backend/target/release/wakewake-agent
```

### 常驻运行（裸机安装）

前台验证成功后 Ctrl+C，安装为 systemd 服务（开机自启 + 断开 ssh 不死）：

```bash
sudo wakewake-agent service install
```

以原用户身份运行（sudo 下自动用 `SUDO_USER`），读取安装时的 `~/.wakewake/config.toml`。

## 配置

`server_url` 和 `pairing_code` 必填，三种方式任选其一（可混用，优先级高者覆盖低者）：

### 方式一：配置文件（推荐长期使用）

搜索路径（按顺序，后者覆盖前者）：

1. `$WAKEWAKE_HOME/config.toml`（默认 `~/.wakewake/config.toml`）
2. `./wakewake.toml`（当前工作目录）
3. `--config <路径>` 显式指定

创建配置文件（参考 [`config.example.toml`](config.example.toml)）：

```toml
# ~/.wakewake/config.toml
server_url   = "https://wakewake.app"
pairing_code = "a1b2c3d4e5f60718"
```

然后直接运行（无需任何参数）：

```bash
wakewake-agent
```

### 方式二：环境变量

```bash
WAKEWAKE_SERVER_URL=https://wakewake.app \
WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
./wakewake-agent
```

适合 Docker / systemd 场景。完整字段映射见 [`config.example.toml`](config.example.toml)。

### 方式三：CLI 参数

```bash
wakewake-agent --server https://wakewake.app --pairing-code a1b2c3d4e5f60718
```

`--help` 查看全部参数。

## 内网自托管（自签 TLS）

若 server 部署在内网并用 Caddy `tls internal`（自签证书），在配置里指向你的 root CA 证书（**信任自己的 CA，而非关闭校验**）：

```toml
# ~/.wakewake/config.toml
[tls]
ca_cert = "/etc/wakewake/caddy-root.crt"
```

root CA 导出（统一镜像 /data 卷内）：

```bash
docker exec <wakewake容器> cat /data/caddy/pki/authorities/local/root.crt > caddy-root.crt
```

预编译二进制与生产镜像**不存在**「信任任何证书」的开关。`danger-insecure-tls` 是编译期 feature，仅 E2E 测试使用（E2E 自行编译携带，不进发布资产）。

## Docker

通过环境变量注入配置，`WAKEWAKE_HOME=/data`（持久化 key.pem）。**必须 `--network host`**：magic packet 走受限广播（RFC 919 不跨网段转发），bridge 网络下广播出不了容器、且不报错（agent 显示成功但目标网卡收不到）：

```bash
docker run -d \
  --name wakewake-agent \
  --network host \
  --restart unless-stopped \
  -e WAKEWAKE_SERVER_URL=https://wakewake.app \
  -e WAKEWAKE_PAIRING_CODE=a1b2c3d4e5f60718 \
  -v wakewake-agent-data:/data \
  jukanntenn/wakewake-agent:latest
```

镜像：`jukanntenn/wakewake-agent`（Docker Hub，多架构 amd64/arm64，随 release tag 发布）。容器内无 systemd，常驻由 `--restart unless-stopped` 承担，无需 `service install`。

## 首次连接

agent 首次连接 server 时会：

1. 生成 RSA-2048 密钥对（持久化到 `$WAKEWAKE_HOME/key.pem`，权限 0600）
2. 通过 SSE 连接的 `X-Public-Key` 头上报公钥，完成配对
3. 之后接收 state/command 事件

pairing code 可在 server 的 agents 页轮换（旧码立即失效）。

## 配置字段

完整字段说明、环境变量映射、默认值见 [`config.example.toml`](config.example.toml)。
