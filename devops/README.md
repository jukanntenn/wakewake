# Build and deployment

English | [中文](README.zh.md)

Design goals: minimal mental overhead + byte-identical containers between staging and prod (differences live only in configuration). Four environments (local acceptance → test → staging → prod), each step is one command, promotion through one-way gates.

## Environment cheat sheet (I want to run it / change config → which one)

| I want to… | Command | Files to edit |
|---|---|---|
| Local dev (hot reload) | `python3 devops/dev.py start` | none (`dev.py` generates `backend/config.local.toml` on first start) |
| Local acceptance of the production shape | `docker compose -f docker/docker-compose.local.yml up -d --build` | `cp docker/config.local.example.toml docker/config.local.toml` (3 secrets); optional `docker/.env` (APP_PORT / POSTGRES_*) |
| Run E2E | `cd e2e && pnpm test` | none (compose ships test secrets) |
| Self-deploy (single-machine user) | `cd docker && docker compose up -d` | `cp config.example.toml config.toml` (public_url + 3 secrets) + `cp .env.example .env` (PG password) |
| Deploy remote test | `ansible-playbook devops/ansible/deploy.yml -l test` | none (`group_vars/test/` is settled) |
| Deploy staging / prod | same with `-l staging` / `-l prod -K` | prod settled (secrets in vault); at staging launch: env.yml (domain) + vault |
| Deploy agent (bare-metal test) | `ansible-playbook devops/ansible/deploy-agent.yml -l test_agent` | none (vault is settled) |
| Deploy agent (Docker route) | compose with `network_mode: host` (contract: [agent-distribution](../specs/backend/agent-distribution.md)) | fn `/vol1/1000/docker/wakewake-agent` (pairing code via rotate) |

Configuration layering (identical across environments): **TOML files first, override with `WAKEWAKE_*` environment variables when needed** (dynamic overrides for e2e/tests go through this layer); secrets live per environment: fixed values in dev / built-in test secrets for e2e / remote ansible vault / self-deploy local files (never committed).

## Environment definitions

| | Local acceptance | test | staging | prod |
|---|---|---|---|---|
| **Where** | this machine | fn (LAN 192.168.5.200) | VPS (not live, placeholder) | VPS abj (59.110.22.138) |
| **Image source** | local build | LAN `192.168.5.50:5000` | Docker Hub `jukanntenn/wakewake` | same as staging |
| **Image tag** | `local` | `main` (floating, = working tree) | `X.Y.Z` (pinned exact version) | same as staging (same version) |
| **In-container Caddy** | `tls internal` | `tls internal` (self-signed) | **HTTP** | **HTTP** (same as staging) |
| **TLS termination** | Caddy self-signed | Caddy self-signed | host tunnel (cloudflared-style) | host Caddy gateway (CF Origin Cert) |
| **Entry** | — | direct LAN | tunnel → host_port | CF CDN → host `:443` gateway (CF CIDRs only) → loopback host_port |
| **debug** | — | all on (easy triage) | off | off |
| **agent** | local | `danger-insecure-tls` | normal TLS | same as staging |
| **Deploy** | compose up | `deploy.yml -l test` | `deploy.yml -l staging` | `deploy.yml -l prod -K` |

Invariants (identical across the four environments): one s6 container (Caddy:`caddy_port` + backend:`backend_port`) + a sibling postgres over a Unix socket (the `postgres-socket` volume). Differences live only in the TLS layer, external ports, image source, and configuration.

## Image tag rules (SemVer 2.0.0, https://semver.org)

Prereleases must use a hyphen (`v0.1.3rc1` is **invalid**, `v0.1.3-rc.1` is valid):

| tag | points to | produced by |
|---|---|---|
| `main` | current working-tree code (floating) | `docker/build.py` (always included, `--tags` only appends) |
| `latest` | newest **stable release** (no prereleases) | CI (git tag triggered, `!is_prerelease`) |
| `X.Y.Z` / `X.Y.Z-rc.N` | the git tag `vX.Y.Z` / `vX.Y.Z-rc.N` | CI; build.py during the manual period |

## Build and release

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

- Local multi-platform goes only through buildx (`--all-platforms` or repeated `--platform`; cross-arch needs QEMU binfmt); CI multi-platform uses native runners (ubuntu-latest + ubuntu-24.04-arm, no QEMU).
- Push mode writes no registry cache (local buildkit cache suffices; registry cache only pays off across orgs, which this project does not do — it would just waste registry disk).
- The `GIT_SHA` build arg is baked into the image → `/api/v1/version` exposes `git_sha` (for deploy verification).

## Promotion flow

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

Rollback: point `wakewake_version` back at the target version and re-run the playbook (images are immutable; a rollback takes seconds). Deploying staging/prod from a non-tag commit adds `-e verify_sha=no` to skip the sha comparison (the health check still runs).

## Quality gates (prek as the single command source, local = CI)

All gate commands are defined in prek, split per directory (workspace mode auto-discovers):

| Config | format group (mutating) | lint group (read-only) | check group |
|---|---|---|---|
| `prek.toml` (root) | builtin fixers, ruff format | actionlint, doc-check (documentation gates) | check-yaml/toml/json, agent-instructions-sync, … |
| `backend/prek.toml` | cargo fmt | cargo fmt --check, cargo clippy | test (real-PG workspace, --lib fallback), build (manual) |
| `frontend/prek.toml` | prettier --write | prettier --check, eslint, tsc | vitest, build (manual) |

- Editor hooks (PostToolUse/Stop) delegate to prek's format/lint groups and never restate commands (no formatter logic lives in `scripts/`).
- Local git hooks: pre-commit = fast gates; pre-push = slow gates (clippy/eslint/tsc/tests); manual = explicit invocation only (the build gate).
- CI invokes prek per `project:hook/group` (`pip install prek==<local version>`), running the same command as local — "local green, CI red" drift is designed out. Common invocations:
  - `prek run --all-files --group format --group lint backend/`
  - `prek run --all-files backend:test` (scripts/backend_tests.py: with PG → full workspace)
  - `prek run --all-files --hook-stage manual frontend:build`
- Node version source of truth: `frontend/.nvmrc`; pnpm version source of truth: the `packageManager` field in `frontend/package.json`.

## Post-deploy health verification

`scripts/check_deploy.py` (run on the control machine, stdlib only):

1. Polls `/api/v1/health` (default 5s interval / 180s timeout) for `{"status":"ok"}`;
2. With `--sha`, compares `/api/v1/version`'s `git_sha` — "container alive but running the wrong image" is exactly what deploy verification must catch;
3. Self-signed environments (test) add `--insecure`.

deploy.yml runs it automatically at the end (health_url / health_insecure are defined per environment in group_vars).

## Ansible layout

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
  files/                           宿主侧脚本（0750 部署到 app_path）：heartbeat.py +
                                   备份三件套（pg-dump-backup / backup-check / backup-drill）
  templates/
    docker-compose.yml.j2          通用（healthcheck / caddy-data 按 tls_profile 分支；
                                   loopback_publish=true 时端口仅回环发布；otel_* 齐
                                   备时渲染 OTLP 遥测 env）
    config.toml.j2                 通用（DSN password urlencode；定义 bootstrap_admin_email
                                   时渲染 [security]，密码走 vault）
    backup.env.j2                  备份脚本凭据（0600；vault 密钥对，b2_key_id 守卫）
    wakewake-heartbeat.service.j2  心跳 systemd 用户单元（0600；KUMA_HEARTBEAT_URL 走 vault）
    wakewake-backup@.service.j2    备份作业 systemd 用户模板单元（%i 选脚本）
    wakewake-backup-*.timer.j2     三个调度 timer（03:30 / 05:30 / 每月 2 号）
    beszel-agent-compose.yml.j2    Beszel agent（hub 在运维观测栈；三变量齐备才部署）
    Caddyfile.test                 test：tls internal + fallback_sni
    wakewake.caddy.j2              宿主 Caddy 网关 site 块（prod）：CF Origin Cert + CF CIDR
                                   放行（remote_ip 守卫）→ 127.0.0.1:host_port
                                   （staging/prod 容器零挂载：直接用镜像内置 /app/Caddyfile）
```

监控/备份的运行手册（拓扑、setup order、清单、DR）：[monitoring.md](monitoring.md)、[backup.md](backup.md)。

### Vault (avpm single-variable encryption)

One vault-id per environment, its password in the avpm keyring (`~/.local/bin/avpm-client`, encrypted multi-device sync); secrets are **committed** as `!vault`-encrypted single variables (group_vars/<env>/vault.yml; ansible.cfg's vault_identity_list decrypts automatically):

```bash
# 接线一个新环境（一次性）：
avpm-client set wakewake-test          # 录入/轮换密码（staging/prod 同理）

# 增改一个 secret（值经 stdin/参数进 encrypt_string，不落明文文件）：
ansible-vault encrypt_string --vault-id wakewake-test@~/.local/bin/avpm-client \
  '<value>' --name db_password >> devops/ansible/group_vars/test/vault.yml
```

`~/.ansible-vault/wakewake-dev.pwd` exists only as a recovery backup should the keyring be lost; delete it once avpm sync is proven reliable.

## Launch state

Prod is wired for abj (host-gateway topology; the console runbook lives in [cloudflare.md](cloudflare.md)). Repo-side items done: prod inventory (`abj`), `host_vars/abj.yml`, prod `env.yml` (domain/version/gateway vars) + vault secrets, GitHub secrets `DOCKERHUB_USERNAME` / `DOCKERHUB_TOKEN` (the token needs read/write/delete, delete to clean up temporary build-<arch> tags). The console steps (DNS record, TLS mode, Origin Cert creation + placement) execute at launch per cloudflare.md. Remaining:

- staging (before it goes live): fill the `staging` group in `hosts.yml`, rename host_vars, set `public_url` / mailer in `group_vars/staging/env.yml`, encrypt the `wakewake-staging` vault per the field list at the top of vault.yml
- prod mailer: disabled at launch — registration stays unverified (purged per `unverified_retention_days`) and password-reset email is unavailable; set SMTP + vault `mailer_smtp_password` when needed
- prod agent: Docker route on fn (`network_mode: host`, contract in [agent-distribution](../specs/backend/agent-distribution.md)); pairing code comes from `POST /agents/default/pairing-code/rotate`
