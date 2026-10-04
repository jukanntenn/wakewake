# 构建与部署体系

[English](README.md) | 中文

设计目标：最低心智负担 + staging 与 prod 容器侧字节同构（差异只在配置）。四环境（本地验收 → test → staging → prod），每步对应一条命令，单向闸门提升。

## 环境速查（我想跑起来/改配置 → 用哪个）

| 我想… | 命令 | 要改的文件 |
|---|---|---|
| 本地开发（热重载） | `python3 devops/dev.py start` | 无（dev.py 首启自动生成 `backend/config.local.toml`） |
| 本地验收生产形态 | `docker compose -f docker/docker-compose.local.yml up -d --build` | `cp docker/config.local.example.toml docker/config.local.toml`（3 个密钥）；可选 `docker/.env`（APP_PORT / POSTGRES_*） |
| 跑 E2E | `cd e2e && pnpm test` | 无（compose 内置测试密钥） |
| 自部署（单机用户） | `cd docker && docker compose up -d` | `cp config.example.toml config.toml`（public_url + 3 密钥）+ `cp .env.example .env`（PG 密码） |
| 部署远程 test | `ansible-playbook devops/ansible/deploy.yml -l test` | 无（`group_vars/test/` 已定型） |
| 部署 staging / prod | 同上 `-l staging` / `-l prod -K` | prod 已定型（secrets 在 vault）；staging 上线时：env.yml（域名）+ vault |
| 部署 agent（bare-metal，test） | `ansible-playbook devops/ansible/deploy-agent.yml -l test_agent` | 无（vault 已定型） |
| 部署 agent（Docker 路线） | compose `network_mode: host`（契约：[agent-distribution](../specs/backend/agent-distribution.zh.md)） | fn `/vol1/1000/docker/wakewake-agent`（配对码走 rotate） |

配置分层规则（所有环境一致）：**TOML 文件为主，必要时用 `WAKEWAKE_*` 环境变量覆盖**（e2e/测试的动态覆盖走这一层）；secrets 按环境落位：dev 固定值 / e2e 内置测试密钥 / 远程 ansible vault / 自部署本地文件（不入库）。

## 环境定义

| | 本地验收 | test | staging | prod |
|---|---|---|---|---|
| **位置** | 本机 | fn (LAN 192.168.5.200) | VPS（未上线，占位） | VPS abj（59.110.22.138） |
| **镜像源** | 本地 build | LAN `192.168.5.50:5000` | Docker Hub `jukanntenn/wakewake` | 同 staging |
| **镜像 tag** | `local` | `main`（浮动，= 工作区代码） | `X.Y.Z`（钉精确版本） | 同 staging（同版本） |
| **容器 Caddy** | `tls internal` | `tls internal`（自签） | **HTTP** | **HTTP**（与 staging 同） |
| **HTTPS 终结** | Caddy 自签 | Caddy 自签 | 宿主机隧道（cloudflared 类） | 宿主 Caddy 网关（CF Origin Cert） |
| **入口** | — | LAN 直连 | 隧道 → host_port | CF CDN → 宿主 `:443` 网关（仅 CF CIDR）→ 回环 host_port |
| **debug** | — | 全开（方便排错） | 关 | 关 |
| **agent** | 本机 | `danger-insecure-tls` | 正常 TLS | 同 staging |
| **部署** | compose up | `deploy.yml -l test` | `deploy.yml -l staging` | `deploy.yml -l prod -K` |

不变量（四环境一致）：单容器 s6（Caddy:`caddy_port` + backend:`backend_port`）+ 兄弟 postgres 走 Unix socket（`postgres-socket` 卷）。差异只在 TLS 层、对外端口、镜像源、配置。

## 镜像 tag 规范（SemVer 2.0.0，<https://semver.org>）

预发布必须连字符（`v0.1.3rc1` **非法**，`v0.1.3-rc.1` 合法）：

| tag | 指向 | 生产者 |
|---|---|---|
| `main` | 当前工作区代码（浮动） | `docker/build.py`（恒定包含，`--tags` 只追加） |
| `latest` | 最新**正式版**（不含预发布） | CI（git tag 触发，`!is_prerelease`） |
| `X.Y.Z` / `X.Y.Z-rc.N` | 对应 git tag `vX.Y.Z` / `vX.Y.Z-rc.N` | CI；手动期 build.py |

## 构建与发布

```bash
# 本地验收（load 到本机，宿主平台）
python3 docker/build.py                       # → wakewake:main
python3 docker/build.py --tags 0.1.3-rc.1     # → wakewake:main + wakewake:0.1.3-rc.1

# 推 LAN registry（test 环境部署前）
python3 docker/build.py --push                # → 192.168.5.50:5000/wakewake:main

# 手动期发布 Docker Hub（CI 启用前的过渡）
python3 docker/build.py --push --registry docker.io --image jukanntenn/wakewake --tags 0.1.3

# CI 期发布（多平台原生 runner：amd64 + arm64 各自构建，imagetools 合并）
git tag v0.1.3 && git push origin v0.1.3      # → 0.1.3 + latest（多平台）
git tag v0.1.3-rc.1 && git push origin v0.1.3-rc.1   # → 0.1.3-rc.1（无 latest）
```

- 本地多平台只能走 buildx（`--all-platforms` 或重复 `--platform`，跨架构需 QEMU binfmt）；CI 多平台走原生镜像（ubuntu-latest + ubuntu-24.04-arm，无 QEMU）。
- push 模式不写 registry 缓存（本地 buildkit 缓存已够；registry cache 只对跨机构建有增益，本项目无此场景，徒占 registry 磁盘）。
- `GIT_SHA` 构建参数烙进镜像 → `/api/v1/version` 暴露 `git_sha`（部署校验用）。

## 提升流程

```
[本地验收]   python3 docker/build.py && docker compose -f docker/docker-compose.local.yml up -d
             # 或直接 dev.py 起开发栈；健康 + 功能确认后
             ↓
git push origin main   ──▶  CI: lint/test/build/e2e（命令源 = prek，见下）
             # CI green
             ↓
[test]
             python3 docker/build.py --push
             ansible-playbook devops/ansible/deploy.yml -l test
             # 部署尾部自动健康校验（/api/v1/health + git_sha 比对本地 HEAD）
             # dogfood 稳定后
             ↓
[staging 发布]（手动期）
             python3 docker/build.py --push --registry docker.io \
               --image jukanntenn/wakewake --tags 0.1.3
             ansible-playbook devops/ansible/deploy.yml -l staging
             ↓（CI 期）
             git tag v0.1.3 && git push origin v0.1.3   # docker-publish.yml 自动发布
             # 改 group_vars/staging/env.yml 的 wakewake_version，重跑 ansible
             ↓ staging 验证通过
[prod 发布]
             # 同版本号：改 group_vars/prod/env.yml 的 wakewake_version，重跑
             # （-K 供 become 任务用）：
             ansible-playbook devops/ansible/deploy.yml -l prod -K
```

回滚：把 `wakewake_version` 改回目标版本，重跑 playbook（镜像不可变，秒级回退）。从非 tag 提交点部署 staging/prod 时 `-e verify_sha=no` 跳过 sha 比对（健康检查仍跑）。

## 质量门禁（prek 单一命令源，本地 = CI）

所有门禁命令定义在 prek，按目录拆分（workspace 模式自动发现）：

| 配置 | format 组（改写型） | lint 组（只读） | check 组 |
|---|---|---|---|
| `prek.toml`（根） | builtin 修正器、ruff format | actionlint、doc-check（文档门控） | check-yaml/toml/json、agent-instructions-sync 等 |
| `backend/prek.toml` | cargo fmt | cargo fmt --check、cargo clippy | test（真 PG workspace，回退 --lib）、build（manual） |
| `frontend/prek.toml` | prettier --write | prettier --check、eslint、tsc | vitest、build（manual） |

- 编辑器 hooks（PostToolUse/Stop）委托 prek 的 format/lint 组，命令不复述（`scripts/` 无格式化逻辑）。
- 本地 git hooks：pre-commit = 快门禁；pre-push = 慢门禁（clippy/eslint/tsc/测试）；manual = 仅显式调用（build 门禁）。
- CI 按 `项目:hook/组` 调 prek（`pip install prek==<本地版本>`），与本地跑同一条命令，杜绝"本地全绿、CI 报红"。常用：
  - `prek run --all-files --group format --group lint backend/`
  - `prek run --all-files backend:test`（scripts/backend_tests.py：有 PG → workspace 全量）
  - `prek run --all-files --hook-stage manual frontend:build`
- Node 版本真源 `frontend/.nvmrc`，pnpm 版本真源 `frontend/package.json` 的 `packageManager`。

## 部署后健康校验

`scripts/check_deploy.py`（控制机执行，stdlib only）：

1. 轮询 `/api/v1/health`（默认 5s 间隔 / 180s 超时）等 `{"status":"ok"}`；
2. 给定 `--sha` 时比对 `/api/v1/version` 的 `git_sha`——容器"活着但跑旧镜像"才是部署验证核心；
3. 自签环境（test）加 `--insecure`。

deploy.yml 尾部自动执行（health_url / health_insecure 每环境 group_vars 定义）。

## Ansible 结构

```
ansible.cfg                        根配置（inventory + avpm vault 身份，仓库任意目录免参数）
devops/ansible/
  ansible.cfg                      目录局部配置（cd 进去跑同样免参数）
  hosts.yml                        inventory（test / test_agent / staging / prod）
  deploy.yml                       统一部署 playbook，--limit 选环境（必填；宿主 Docker
                                   由运维预装）+ 网关 site 安装 + 尾部健康校验
  deploy-agent.yml                 agent 部署（本地构建 + supervisor 常驻；test 专用）
  group_vars/
    all.yml                        共享变量（端口、PG 库名/用户、路径）
    test/{env.yml,vault.yml}       test：LAN registry 浮动 main + 自签 HTTPS + secrets（加密）
    staging/{env.yml,vault.yml}    staging：Docker Hub 钉版本 + 隧道前置（占位）
    prod/{env.yml,vault.yml}       prod：Docker Hub 钉版本 + 宿主 Caddy 网关前置 + secrets（加密）
  host_vars/                       每主机事实（user / home）
  templates/
    docker-compose.yml.j2          通用（healthcheck / caddy-data 按 tls_profile 分支；
                                   loopback_publish=true 时端口仅回环发布）
    config.toml.j2                 通用（DSN password urlencode；定义 bootstrap_admin_email
                                   时渲染 [security]，密码走 vault）
    Caddyfile.test                 test：tls internal + fallback_sni
    wakewake.caddy.j2              宿主 Caddy 网关 site 块（prod）：CF Origin Cert + CF CIDR
                                   放行（remote_ip 守卫）→ 127.0.0.1:host_port
                                   （staging/prod 容器零挂载：直接用镜像内置 /app/Caddyfile）
```

### Vault（avpm 单变量加密）

每环境一个 vault-id，密码在 avpm keyring（`~/.local/bin/avpm-client`，多设备加密同步），secrets 以 `!vault` 单变量加密**入库**（group_vars/<env>/vault.yml，ansible.cfg 的 vault_identity_list 自动解密）：

```bash
# 接线一个新环境（一次性）：
avpm-client set wakewake-test          # 录入/轮换密码（staging/prod 同理）

# 增改一个 secret（值经 stdin/参数进 encrypt_string，不落明文文件）：
ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client \
  '<value>' --name db_password >> devops/ansible/group_vars/test/vault.yml
```

`~/.ansible-vault/wakewake-dev.pwd` 仅作 keyring 丢失时的恢复备份，确认 avpm 同步可靠后可删。

## 上线状态

prod 已在仓库侧接线完成（abj，宿主网关拓扑；控制台操作手册在 [cloudflare.zh.md](cloudflare.zh.md)）。仓库侧已完成：prod inventory（`abj`）、`host_vars/abj.yml`、prod `env.yml`（域名/版本/网关变量）+ vault secrets、GitHub secrets `DOCKERHUB_USERNAME` / `DOCKERHUB_TOKEN`（token 需带 read/write/delete 权限，delete 用于清理临时 build-<arch> tag）。控制台步骤（DNS 记录、TLS 模式、Origin Cert 生成与落盘）按 cloudflare.zh.md 在上线时执行。剩余事项：

- staging（上线前）：`hosts.yml` 填 staging group、host_vars 改名、`group_vars/staging/env.yml` 填 `public_url` / 邮件、按 vault.yml 头部字段清单加密 `wakewake-staging` vault
- prod 邮件：上线时未开通——注册保持未验证态（按 `unverified_retention_days` 清理），密码重置邮件不可用；需要时填 SMTP + vault 加密 `mailer_smtp_password`
- prod agent：fn 上的 Docker 路线（`network_mode: host`，契约见 [agent-distribution](../specs/backend/agent-distribution.zh.md)）；配对码从 `POST /agents/default/pairing-code/rotate` 获取
