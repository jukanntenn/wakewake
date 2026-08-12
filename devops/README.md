# 部署验收体系

四环境（本地验收 → 远程验收 dev → staging → prod），每步对应一条命令，单向闸门提升。
设计目标：最低心智负担 + staging 与 prod 容器侧字节同构（差异只在配置）。

## 环境定义

| | 本地验收 | 远程验收 dev | staging | prod |
|---|---|---|---|---|
| **位置** | 本机 | fn (LAN) | VPS | VPS |
| **镜像源** | 本地 build | LAN `192.168.5.50:5000` | Docker Hub `jukanntenn/wakewake` | 同 staging |
| **镜像 tag** | `local` | `main`（浮动） | `vX.Y.Z`（钉精确） | 同 staging（同版本） |
| **容器 Caddy** | `tls internal` | `tls internal` | **HTTP** | **HTTP** |
| **HTTPS 终结** | Caddy 自签 | Caddy 自签 | 外部反代 | 外部反代 + CF CDN |
| **agent** | 本机 | `danger-insecure-tls` | 正常 TLS | 同 staging |
| **部署** | compose up | ansible `-l dev` | ansible `-l staging` | ansible `-l production` |

不变量（四环境一致）：单容器 s6（Caddy:`caddy_port` + backend:`backend_port`）+ 兄弟 postgres 走 Unix socket（`postgres-socket` 卷）。差异只在 TLS 层、对外端口、镜像源、配置。

## 提升流程

```
[本地验收]   docker compose -f docker/docker-compose.local.yml up -d --build
             # 健康 + 功能确认后
             ↓
git push origin main   ──▶  CI: lint/test/build/e2e（已有，不动）
             # CI green
             ↓
[远程验收 dev]
             python3 docker/build.py --push
             cd devops/ansible && ansible-playbook -i hosts.yml deploy.yml -l dev \
               --vault-password-file ~/.ansible-vault/wakewake-dev.pwd
             # dogfood 稳定
             ↓
[staging 发布]（手动期）
             python3 docker/build.py --push --registry docker.io \
               --image jukanntenn/wakewake --base-tag v0.1.0
             ansible-playbook -i hosts.yml deploy.yml -l staging \
               --vault-password-file ~/.ansible-vault/wakewake-staging.pwd
             ↓（CI 启用后）
[staging 发布]（CI 期）
             git tag v0.1.0 && git push --tags   # docker-publish.yml 自动 build+push 语义化标签
             # 改 group_vars/staging.yml 的 wakewake_version，重跑 ansible
             ↓ staging 验证通过
[prod 发布]
             ansible-playbook -i hosts.yml deploy.yml -l production \
               --vault-password-file ~/.ansible-vault/wakewake-prod.pwd
             # 同版本号，仅换配置 + 目标 host
```

## Tag 约定（metadata-action 语义化）

CI（`.github/workflows/docker-publish.yml`）由 `git tag v*` 触发，docker/metadata-action 自动生成：

| 触发 | 生成的标签 | 用途 |
|---|---|---|
| `git tag v0.1.0` | `0.1.0`, `0.1`, `0`, `latest`, `sha-<short>` | 正式发布 |
| `git tag v0.1.0-rc.1` | `0.1.0-rc.1`, `sha-<short>` | 预发布（不产 `latest`/滚动 tag） |

- staging/prod ansible 钉 `wakewake_version: "vX.Y.Z"` -> 镜像 `jukanntenn/wakewake:X.Y.Z`。
- 回滚：改 `wakewake_version` 指向旧版本，重跑 playbook（镜像不可变，秒级回退）。

手动期（CI 未启用）：`docker/build.py --push --registry docker.io --image jukanntenn/wakewake --base-tag vX.Y.Z` 直接推单架构到 Docker Hub。

## Ansible 结构

```
devops/ansible/
  hosts.yml                    inventory（dev / dev_agent / staging / production 四 group）
  deploy.yml                   单 playbook，env-agnostic，--limit 选环境（必填）
  group_vars/
    all.yml                    共享变量（端口、PG 库名/用户、路径）
    dev.yml                    dev：LAN registry，浮动 main tag，tls internal
    staging.yml                staging：Docker Hub 钉版本，HTTP，外部反代
    production.yml             prod：与 staging 同构，差异仅配置
  host_vars/
    fn.yml / agent.yml         dev host 事实
    staging.yml / production.yml   TODO 占位（host 定后改名）
  vars/
    dev/vault.yml              dev secrets（加密入库）
    staging/vault.yml.example  staging secrets 模板（真实 vault.yml 加密不入库）
    production/vault.yml.example
  templates/
    docker-compose.yml.j2      通用（healthcheck / caddy-data 按 tls_profile 分支）
    config.toml.j2             通用（DSN password 用 urlencode）
    Caddyfile.dev              dev：tls internal + fallback_sni
    Caddyfile.http             staging + prod 共用：plain HTTP
```

### Vault 密码文件约定

每环境一个 vault 密码文件（本地保管，不入库）：
- `~/.ansible-vault/wakewake-dev.pwd`
- `~/.ansible-vault/wakewake-staging.pwd`
- `~/.ansible-vault/wakewake-prod.pwd`

新建 staging/prod vault：
```bash
cd devops/ansible
cp vars/staging/vault.yml.example vars/staging/vault.yml
# 编辑填入真实 secrets
ansible-vault encrypt vars/staging/vault.yml \
  --vault-password-file ~/.ansible-vault/wakewake-staging.pwd
```

## 待办（host 就绪后填）

- `hosts.yml`：staging / production group 填入真实 VPS IP / 用户
- `host_vars/staging.yml` / `production.yml`：重命名为对应 host 名，填 `user`/`home`
- `group_vars/staging.yml` / `production.yml`：填 `public_url`（真实域名）、按需开 `mailer_*`
- 真实 `vars/staging/vault.yml` / `vars/production/vault.yml`：按 `.example` 填 + 加密
- GitHub secrets：`DOCKERHUB_USERNAME` / `DOCKERHUB_TOKEN`（CI 启用时）
