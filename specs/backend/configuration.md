# 配置机制规范（server + agent + 部署面）

> 本文是 wakewake 配置的唯一规范：加载分层、server/agent schema、环境变量映射、
> example 文件约定，以及各部署环境"配置写在哪、secrets 放哪"的全景。
> 运维操作流程（构建/发布/提升闸门）见 [`../../devops/README.md`](../../devops/README.md)，本文只管机制与不变量。

## 1. 设计原则

1. **TOML 文件为主，环境变量覆盖，CLI 参数最高**。部署级配置写进 `config.toml`
   （一次配置、可 diff、可 review）；`WAKEWAKE_*` 环境变量用于容器注入与测试动态覆盖；
   CLI 参数用于调试一次性覆盖。
2. **server 与 agent 同架构、分文件**：两份独立 `Settings`（字段不同），共用同一套
   规约（config-rs + clap + 三层覆盖 + `WAKEWAKE_` 前缀 + `__` 嵌套分隔）。
3. **fail-fast**：配置校验失败（garde）→ 启动失败，不降级运行。
4. **example 文件是主要用户文档**（见 §7），schema 真源是 `Settings` struct +
   builder `set_default`（server：`crates/server/src/config.rs`；agent：
   `crates/agent/src/config.rs`）。

技术选型：config-rs 0.15（`default-features = false, features = ["toml"]`，裁掉
yaml/json/ron parser）+ clap 4 derive（CLI 解析与 `--help`，config-rs 不替代 clap，
职责正交）。

## 2. 三层覆盖模型（优先级从高到低）

```
CLI 参数（clap → set_override_option）  >  环境变量（WAKEWAKE_*）  >  TOML 文件  >  内置默认值
```

| 层 | 实现 | 用途 |
|---|---|---|
| 内置默认值 | `Config::builder().set_default(key, value)` | 安全默认（端口、TTL 等不敏感项） |
| TOML 文件 | `File::with_name(path).required(false/true)` | 部署级配置（主体载体） |
| 环境变量 | `Environment::with_prefix("WAKEWAKE").prefix_separator("_").separator("__").try_parsing(true)` | 容器注入（DSN 拼接）、e2e/测试动态覆盖 |
| CLI 参数 | `set_override_option(key, clap_value)` | 调试/一次性覆盖 |

加载顺序细节（server `config.rs::Settings::load`）：

- **文件搜索**：`--config <path>` 指定时必须存在（`required(true)`）；未指定时搜
  工作目录 `./config.toml`（`File::with_name("config").required(false)`，找不到不报错）。
- **`__` 而非 `_` 做嵌套分隔**：单 `_` 与 TOML key 内下划线冲突（如
  `packet_delay_ms` 会被误拆为嵌套）。`WAKEWAKE_SERVER__HOST` → `server.host`。
- **`try_parsing(true)`**：env 字符串尝试解析为 `i64`/`f64`/`bool`（`"8080"` →
  `i64` → `u16`；解析失败保持字符串），bool 支持 `true/false/1/0`。
- **`set_override_option`**：clap 未传（`None`）不覆盖，传了即最高优先级。
- **agent 的顶层键是单 `_`**：`WAKEWAKE_SERVER_URL` / `WAKEWAKE_PAIRING_CODE` /
  `WAKEWAKE_HOME`（顶层无嵌套，落在 `__` 之外）。

## 3. Server 配置 schema

### `[app]`（必填）

| 字段 | 类型 | env | 说明 |
|---|---|---|---|
| `public_url` | string | `WAKEWAKE_APP__PUBLIC_URL` | 对外公网地址（scheme+host[+port]）。拼接邮件链接/agent 文档地址。**不从请求 Host 推导**（反代场景可伪造）。缺失启动失败 |

### `[server]`

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `host` | string | `0.0.0.0` | `WAKEWAKE_SERVER__HOST` | 监听地址 |
| `port` | u16 | `8080` | `WAKEWAKE_SERVER__PORT` | **不变量 8080**：镜像内 `docker/caddy/routes.caddy` 反代 `127.0.0.1:8080`，永不改 |
| `trust_proxy` | bool | `true` | `WAKEWAKE_SERVER__TRUST_PROXY` | true=从 XFF 最左取客户端 IP（反代）；false=TCP 对端（直连部署） |

### `[database]`（必填）

| 字段 | 类型 | env | 说明 |
|---|---|---|---|
| `dsn` | string | `WAKEWAKE_DATABASE__DSN` | PG 连接串。容器拓扑统一走 Unix socket：`postgres:///db?host=/var/run/postgresql&user=...&password=...&sslmode=disable` |

DSN 单一来源规则（密码只写一处）：

- 自部署/本地验收：compose 从 `.env` 的 `POSTGRES_*` 插值拼接注入 env；
- ansible 环境：`config.toml.j2` 渲染进 TOML，密码经 `urlencode`（vault 值可含 URL 特殊字符）；
- e2e：compose 内置独立测试凭据。

### `[jwt]`

| 字段 | 类型 | 默认 | env | 校验 | 说明 |
|---|---|---|---|---|---|
| `signing_key` | string | — | `WAKEWAKE_JWT__SIGNING_KEY` | `length(min=32)` | access HMAC 密钥 |
| `refresh_signing_key` | string | — | `WAKEWAKE_JWT__REFRESH_SIGNING_KEY` | `length(min=32)` | refresh HMAC 密钥（独立，勿复用） |
| `access_expire` | humantime | `15m` | `WAKEWAKE_JWT__ACCESS_EXPIRE` | — | e2e short-ttl 用 `10s` 覆盖 |
| `refresh_expire` | humantime | `720h` | `WAKEWAKE_JWT__REFRESH_EXPIRE` | — | 30d |

密钥无默认值，生成命令 `openssl rand -base64 32`。

### `[password_reset]`

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `secret` | string | — | `WAKEWAKE_PASSWORD_RESET__SECRET` | 无状态 reset token HMAC 密钥（必填） |
| `expire` | humantime | `1h` | `WAKEWAKE_PASSWORD_RESET__EXPIRE` | token TTL |

### `[pow]`

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `difficulty` | u8 | `4` | `WAKEWAKE_POW__DIFFICULTY` | 前导零个数 |
| `challenge_ttl` | humantime | `10m` | `WAKEWAKE_POW__CHALLENGE_TTL` | challenge 有效期 |

### `[mailer]`

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `enabled` | bool | `false` | `WAKEWAKE_MAILER__ENABLED` | false 时密码重置端点返 503 |
| `smtp_host` | Option | — | `WAKEWAKE_MAILER__SMTP_HOST` | SMTP 服务器 |
| `smtp_port` | u16 | `587` | `WAKEWAKE_MAILER__SMTP_PORT` | SMTP 端口 |
| `smtp_username` | Option | — | `WAKEWAKE_MAILER__SMTP_USERNAME` | 用户名 |
| `smtp_password` | Option | — | `WAKEWAKE_MAILER__SMTP_PASSWORD` | 密码 |
| `from_address` | Option | — | `WAKEWAKE_MAILER__FROM_ADDRESS` | 发件人地址 |
| `from_name` | string | `WakeWake` | `WAKEWAKE_MAILER__FROM_NAME` | 发件人名 |

### `[rate_limit]`（测试专用）

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `disabled` | bool | `false` | `WAKEWAKE_RATE_LIMIT__DISABLED` | true = 旁路 axum-governor 速率限制。**仅旁路频率限制，不旁路业务配额**（`MAX_DEVICES_PER_USER` 等硬编码 const）。生产恒 false；e2e/压测 true |

### `[security]`（bootstrap admin）

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `bootstrap_admin_email` | string | `admin@wakewake.local` | `WAKEWAKE_SECURITY__BOOTSTRAP_ADMIN_EMAIL` | 启动时幂等创建（不存在才建，存在不覆盖）。类比 Grafana 默认 admin |
| `bootstrap_admin_password` | string | `wakewake123` | `WAKEWAKE_SECURITY__BOOTSTRAP_ADMIN_PASSWORD` | ⚠️ 生产务必覆盖默认值 |

### `[log]`

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `level` | string | `info` | `WAKEWAKE_LOG__LEVEL` | **优先级：`RUST_LOG`（支持按模块 `wakewake_server=debug,sqlx=warn`）> 本项**。serve 与 admin 子命令同用此回落链 |
| `dir` | string | `data/logs` | `WAKEWAKE_LOG__DIR` | tracing-appender daily rolling 目录（4 文件：app/traces/metrics/logs）。容器内 `./data:/app/data` 已挂载即持久化 |

### `[maintenance]`（运维止血，初值）

| 字段 | 类型 | 默认 | env | 说明 |
|---|---|---|---|---|
| `enabled` | bool | `false` | `WAKEWAKE_MAINTENANCE__ENABLED` | 配置文件只决定**初值**；运行时经 `POST /admin/maintenance` 切换并持久化到 `data/maintenance.json`（重启恢复） |
| `mode` | enum | `registration_disabled` | `WAKEWAKE_MAINTENANCE__MODE` | `registration_disabled` / `readonly` / `full`（admin 恒放行） |
| `message` | string | 英文固定文案 | `WAKEWAKE_MAINTENANCE__MESSAGE` | 拦截响应文案 |

### 非 `WAKEWAKE_` 前缀的环境变量

| 变量 | 消费者 | 说明 |
|---|---|---|
| `RUST_LOG` | tracing EnvFilter | 服务日志级别最高优先级（见 `[log].level`） |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | observability | 设了走 OTLP collector，不设走本地文件 exporter |
| `TZ` | s6 `cont-init.d/01-setup.sh` | 容器时区 |

### CLI（`wakewake-server`）

`--config <path>`；子命令：`serve [--host] [--port]`（缺省行为）、`version`、
`admin create|promote|demote|list`（admin 子命令日志走同一 `log.level` 回落链）。

> server 的 `Settings` **不含 `[wol]`**——server 永不发 magic packet。

## 4. Agent 配置 schema

Agent 与 server 同框架，字段不同。文件搜索路径（按序，后者覆盖前者）：
`$WAKEWAKE_HOME/config.toml`（默认 `$HOME/.wakewake/`）→ `./wakewake.toml` →
`--config <path>` 显式指定（必须存在，跳过前两者）。

```toml
# agent 的 config.toml
server_url   = "https://wakewake.app"     # 必填；内网自签需编译 --features danger-insecure-tls
pairing_code = "a1b2c3d4e5f60718"         # 必填，从 server agents 页获取

[wol]
broadcast_addr  = "255.255.255.255:9"
packet_count    = 3
packet_delay_ms = 50

[log]
level  = "info"      # 优先级 RUST_LOG > 本项
format = "pretty"    # pretty（终端）/ json（容器）；agent 无 dir 键，默认 <home_dir>/logs

[bemfa]
broker   = "bemfa.com"
port     = 9503
# api_base = "http://bemfa-mock:8080"   # 覆盖缝隙，日常不设
```

| 组 | 字段 | env | 说明 |
|---|---|---|---|
| 顶层（必填） | `server_url` / `pairing_code` | `WAKEWAKE_SERVER_URL` / `WAKEWAKE_PAIRING_CODE` | CLI `--server` / `--pairing-code` 最高 |
| 顶层 | `home_dir` | `WAKEWAKE_HOME` | 存 config.toml + key.pem + logs。优先级 env > `$HOME/.wakewake` > `./.wakewake`；容器 `/data`（卷 `agent_data`）；ansible supervisor 指向 `app_path` |
| `[wol]` | `broadcast_addr` / `packet_count` / `packet_delay_ms` | `WAKEWAKE_WOL__*` | magic packet 发送参数 |
| `[log]` | `level` / `format` / `dir`(Option) | `WAKEWAKE_LOG__*` | `dir` 未设 = `<home_dir>/logs` |
| `[bemfa]` | `broker` / `port` / `api_base`(Option) | `WAKEWAKE_BEMFA__BROKER` / `__PORT` / `__API_BASE` | 巴法云端点 |

`[bemfa].api_base` 语义（重要）：真实巴法云各 API 分散在多个域名
（`pro.bemfa.com` / `apis.bemfa.com`），无法用单值做默认——**不设 = 按端点用真实
各域名默认；设了 = 整体基址覆盖 + 路径重组**（`{api_base}/v1/createTopic` 等）。
该键的唯一用途是 E2E/测试指向 mock（`docker-compose.e2e.yml` 指向 bemfa-mock）。
`port == 9503` 时 MQTT 走 TLS（系统默认 CA），非 9503（如 mosquitto 1883）走明文。

## 5. 校验与 fail-fast

加载后用 garde 校验语义约束（config-rs 不管校验）：

- `app.public_url`：`#[garde(url)]`；JWT 两密钥 `length(min = 32)`；`Settings` 整体 `dive`。
- 校验失败 → 进程 exit 1，不降级运行。

| 失败类型 | 行为 |
|---|---|
| `--config` 指定的文件不存在 | 启动失败，错误含路径 |
| TOML 语法错误 | 启动失败，报错带行号 |
| 必填字段缺失（无文件无 env） | 启动失败，`missing field` |
| 密钥长度不足 | 启动失败，`length min = 32` |
| env 值类型错（如 port 非数字） | 启动失败，`invalid type` |

时间字段（`access_expire` 等）为 humantime 字符串，`Settings` 访问器里
`humantime::parse_duration` 解析（`15m` / `720h` / `10s`）。

## 6. 环境变量映射规则

| 规则 | 说明 | 示例 |
|---|---|---|
| 前缀 | `WAKEWAKE_` | — |
| 嵌套分隔 | `__` | `WAKEWAKE_SERVER__HOST` → `server.host` |
| agent 顶层键 | 单 `_`（无嵌套） | `WAKEWAKE_SERVER_URL` → `server_url` |
| 大小写 | env 全大写 → TOML 全小写 | `WAKEWAKE_DATABASE__DSN` → `database.dsn` |
| 类型推断 | `try_parsing(true)` 尝试 i64/f64/bool | `WAKEWAKE_SERVER__PORT=8080` → u16 |
| 特殊字符 | env 值整体是字符串，无需转义 | `WAKEWAKE_DATABASE__DSN="postgres://u:p@db:5432/d"` |

## 7. `config.example.toml` 约定（用户面向文档）

example 文件是**主要用户文档**，schema 变更必须同步它。现有三份，读者不同：

| 文件 | 读者 | 内容 |
|---|---|---|
| `backend/config.example.toml` | 全量 schema 文档 | 全部字段 + 说明 |
| `backend/crates/agent/config.example.toml` | agent 用户 | agent 全量字段 |
| `docker/config.example.toml` | 自部署用户 | 仅自部署需关心的项（public_url + 3 密钥 + 可选 mailer） |
| `docker/config.local.example.toml` | 本地验收 | 全量字段显式列出（默认值/占位符），`cp` 后改密钥 |

字段约定：

| 字段类型 | 约定 |
|---|---|
| 必填无默认（如 `jwt.signing_key`） | 不注释，值占位符 `"CHANGE_ME..."`，标 `[REQUIRED]` |
| 可选有默认（如 `server.port`） | 注释掉，值为默认值，标 `[OPTIONAL]` |
| 安全敏感默认 | 标 `⚠️` 警告 |

每个字段必须有：一行说明、`[REQUIRED]`/`[OPTIONAL]` 标签、`Env:` 标签、
`Default:` 标签（仅可选字段）。

## 8. 部署面配置全景（每环境读什么）

原则：**每个环境 = 一条命令 + 至多一个要改的文件**。运维速查表在
[`../../devops/README.md`](../../devops/README.md)（环境速查）。

| 环境 | 入口命令 | 配置载体 | secrets 落位 |
|---|---|---|---|
| 本地开发 | `python3 devops/dev.py start` | dev.py 首启生成 `backend/config.local.toml`（内嵌 dev 默认值） | 固定 dev 值（不入库） |
| 本地验收 | `docker compose -f docker/docker-compose.local.yml up` | `docker/config.local.toml`（模板 `config.local.example.toml`）+ compose env（DSN/public_url 插值） | 占位密钥（不入库） |
| E2E | `cd e2e && pnpm test` | `e2e/docker-compose.e2e.yml` 纯 env（覆盖层当主层用）+ `short-ttl.yml` 覆盖 | compose 内置测试密钥（与生产绝不复用） |
| 自部署 | `cd docker && docker compose up -d` | `docker/config.toml`（模板 `config.example.toml`）+ `.env`（`POSTGRES_*`） | 用户本地文件（不入库） |
| 远程 test/staging/prod | `ansible-playbook devops/ansible/deploy.yml -l <env>` | `group_vars/<env>/env.yml` + `vault.yml`（avpm 单变量加密）→ 渲染 `config.toml.j2` + `docker-compose.yml.j2` + Caddyfile | ansible vault（入库但加密） |
| agent（bare-metal） | `deploy-agent.yml -l test_agent` | vault 的 `agent_server_url`/`agent_pairing_code` → 渲染 `agent-config.toml.j2` + supervisord conf | ansible vault |

ansible 环境变量（`group_vars/<env>/env.yml`）：`image`（test=LAN registry 浮动
`main`；staging/prod=`wakewake_version` 钉版本）、`host_port`、`public_url`、
`health_url`/`health_insecure`、`tls_profile`（http|https，分支 healthcheck 与
caddy-data 卷）、`caddyfile_template`（未定义 = 零挂载，用镜像内置 Caddyfile）、
`mailer_*`。共享不变量在 `group_vars/all.yml`（8080/8443、PG 库名/用户、路径）。

### Caddyfile：4 份站点变体 + 1 份路由真源

路由唯一真源是 `docker/caddy/routes.caddy`（烤进镜像 `/app/caddy/routes.caddy`）：
API/SSE 反代 + 静态前端 + 安全头 + 访问日志。**改路由只改这一处**。每环境 Caddyfile
只做两件事：import 路由 + 声明站点地址与 TLS 层：

| 变体 | TLS 形态 | 使用环境 |
|---|---|---|
| `docker/Caddyfile`（镜像内置 `/app/Caddyfile`） | hostless `:8443` 纯 HTTP | 自部署 + staging（零挂载） |
| `docker/Caddyfile.local` | `tls internal` 自签（SAN localhost, app） | 本地验收 + e2e（共用） |
| `devops/ansible/templates/Caddyfile.test` | `tls internal` + `fallback_sni`（IP 直连） | test |
| `devops/ansible/templates/Caddyfile.prod` | CF Origin Cert（占位） | prod |

## 9. 不变量清单（跨环境恒成立）

改下列任何一项 = 多处联动，动前先 grep 全部引用点：

1. **8080**：backend 监听 = `routes.caddy` 反代目标（`[server].port` 永不改）。
2. **8443**：容器内 Caddy 监听（compose `ports` 的容器侧、healthcheck、Caddyfile 站点地址）。
3. **`/var/run/postgresql`**：postgres-socket 共享卷路径 = DSN `host=` = postgres
   healthcheck `-h`（postgres 镜像默认，勿改）。
4. **`/app/Caddyfile`**：s6 caddy run 脚本固定读取路径（挂载点必须一致）。
5. **`/app/config.toml`**：server 默认文件搜索路径（工作目录 `/app`）。
6. **`./data:/app/data`**：日志轮转 + `maintenance.json` 持久化根。
7. **服务名 `app` / `mailpit` / `postgres`**：docker DNS 被另一处引用
   （Caddyfile SAN、`smtp_host`、depends_on），改名必须同步。
8. **compose 宿主端口与 `public_url` 同源**：本地验收 `APP_PORT` 同时插值
   `ports` 与 `WAKEWAKE_APP__PUBLIC_URL`；远程环境 `host_port` ↔ `public_url` 在
   env.yml 成对出现。

## 10. 密钥管理

- **生成**：`openssl rand -base64 32`；三把 HMAC 密钥（jwt access / jwt refresh /
  password_reset）彼此独立，勿复用。
- **开发/e2e**：固定测试值（e2e 与生产绝不复用）。
- **自部署**：本地 `config.toml` + `.env`，均不入库（`*.local.toml`、`.env` 已 ignore）。
- **远程环境**：avpm vault 单变量加密入库（轮换见 devops/README §Vault）。
- **轮换**：改值重启即生效。JWT 密钥轮换 → 现存 token 全失效（预期，强制重登）。
- **bootstrap admin**：默认 `admin@wakewake.local` / `wakewake123` 仅幂等首建；
  生产必须经 env/TOML 覆盖默认密码。
