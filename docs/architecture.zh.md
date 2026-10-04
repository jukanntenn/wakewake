# wakewake 架构

[English](architecture.md) | 中文

改动源码树之前先读本文。它是代码库的有序地图——组件、边界、以及新行为的归属；决策理由在链接的 RFC（决策记录）里。

## 这个包是什么

wakewake 交付一套网络唤醒（WoL）服务：Rust 后端（axum + sqlx + tokio）加独立的 WoL agent 二进制，以及静态导出的 Next.js 16 前端，三者共同打包为单个多架构 Docker 镜像、部署在 Caddy 之后。运营者将其跑在面向 10 万并发 SSE 连接规划的 2 GB VPS 上；最终用户通过 Web 界面唤醒和管理机器，agent 经 SSE 与后端保持连接。

## 组件

| 组件 | 职责 | 公共接口 |
|---|---|---|
| `backend/crates/protocol` | server 与 agent 共享的线上类型 | Rust crate `protocol` |
| `backend/crates/server` | HTTP API、SSE hub、持久化 | REST 端点 + SSE 流 |
| `backend/crates/agent` | 目标机器上的独立唤醒客户端 | 二进制 `wakewake-agent` |
| `backend/crates/seed` | 压测数据装载 | Rust crate `seed` |
| `frontend/` | Web 界面（静态导出） | 托管的静态站点 |
| `docker/` `devops/` | 镜像构建；开发环境与部署 | `docker/build.py`、`dev.py`、ansible |
| `.agents/rfcs/` `.agents/skills/` | 决策记录；agent 工作流 | hdsh 门禁与 skills |

## 新行为去哪里

API 或 hub 行为进 `backend/crates/server`；线上格式变更从 `protocol` 开始；agent 侧行为进 `backend/crates/agent`；界面进 `frontend/`；schema 变更以可回滚的 `sqlx migrate add -r` 迁移对落到 `backend/migrations`。每个非平凡改动都在 `.agents/rfcs/` 下新增或更新一条决策记录。归属参考文档是[文档标准](AGENTS.md)、[双语文档契约](i18n/README.zh.md)与 [RFC 规则](../.agents/rfcs/README.zh.md)。

贡献者入口见 [development.md](development.zh.md)。
