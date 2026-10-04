# WakeWake Specs（工程规范）

[English](README.md) | 中文

提交入库的设计规范。规则：

- 规范描述**机制与不变量**（为什么这样设计、哪些东西不能动）；操作流程归 [`../devops/README.zh.md`](../devops/README.zh.md)，代码本身归源码注释。
- 改动相关实现时同步对应 spec；spec 与实现漂移视为 bug。
- 新增 spec 文件意味着同一变更中在本表两个索引各补一行；决策历史归 [`.agents/rfcs/`](../.agents/rfcs/README.zh.md)。

## 目录

| 文件 | 内容 |
|---|---|
| [`backend/agent-distribution.zh.md`](backend/agent-distribution.zh.md) | Agent 分发：GitHub Releases 二进制 + Docker Hub 镜像、install.sh 一行安装契约、`service install` 子命令、自签 TLS 的 `tls.ca_cert`、`--network host` 硬前提 |
| [`backend/configuration.zh.md`](backend/configuration.zh.md) | 配置机制：三层覆盖、server/agent schema、env 映射、example 约定、部署面全景与不变量 |
| [`backend/risk-controls.zh.md`](backend/risk-controls.zh.md) | admin 运行时风控：运行时句柄家族（maintenance/mailer/pow/ip-bans）、分路邮件预算与降级语义、未验证账号清理不变量、IP 封禁匹配与 fail-open 规则、风控面板数据来源 |
| [`frontend/agent-onboarding.zh.md`](frontend/agent-onboarding.zh.md) | Agent onboarding 三态页 + 设备添加前置门控：配额经 `UserPublic.limits` 下发、pending/offline 语义、命令模板槽、安全上下文拦截 |
| [`testing/load.zh.md`](testing/load.zh.md) | 负载与容量测试：两套装置（裸机 / prod-sim）、含出口整形的 2c2g3Mbps 环境契约、R1–R7 场景矩阵、seed 数据契约、红线、拐点/甜点判定方法学 |
