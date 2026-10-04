# 开发指南

[English](development.md) | 中文

安装教程带新贡献者从前置条件走到可校验的检出；随后的贡献者参考覆盖日常工作流与仓库的工具集成。

## 安装教程

### 前置条件

- Rust 1.95+（edition 2021 workspace），经 [rustup](https://rustup.rs) 安装
- Node.js 20+ 与 [pnpm](https://pnpm.io)，服务 frontend 与 e2e 两个 workspace
- Python 3.12+ 负责开发编排，[uv](https://docs.astral.sh/uv/) 承载 `hdsh` 门禁，Docker 与 compose 提供 PostgreSQL 17 + Mailpit

### 首次安装

`python3 devops/dev.py start` 拉起 Docker 基础设施（postgres + mailpit）、后端（`cargo watch`）与前端（`pnpm dev`），全程健康检查；首次运行会生成 `backend/config.local.toml`。dev.py 报告所有服务健康即安装完成。`dev.py init` 只启动基础设施；`dev.py stop` 停止。

## 贡献者参考

### 项目布局

一个名字在目录、接口与 id 前缀三处指同一概念；完整组件地图见 [architecture.md](architecture.zh.md)。

### 日常命令

- `prek run <hook> [--all-files]` —— 仓库门禁集，按 format / lint / check 分组
- `cargo test -p <crate>` —— 后端单元与集成测试，跑在真实 PostgreSQL 上
- `pnpm -C frontend lint` / `pnpm -C frontend test:run` —— 前端静态检查与单元测试
- `hdsh pairing verify`、`hdsh rfc verify`、`hdsh adopt verify` —— harness 门禁；其语料按[双语文档契约](i18n/README.zh.md)配置在 `.hdsh/docs.manifest.json` 与 `.hdsh/pairing.manifest.json`
- `python3 docker/build.py` 与 `ansible-playbook devops/ansible/deploy.yml -l <env>` —— 镜像构建与环境部署

### TODO 标记

本仓库没有按紧迫度分级的 TODO 约定；请开 issue 或 RFC，不要在树里留 TODO 标记。
