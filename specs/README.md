# WakeWake Specs（工程规范）

提交入库的设计规范。规则：

- 规范描述**机制与不变量**（为什么这样设计、哪些东西不能动）；操作流程归
  [`../devops/README.md`](../devops/README.md)，代码本身归源码注释。
- 改动相关实现时同步对应 spec；spec 与实现漂移视为 bug。

## 目录

| 文件 | 内容 |
|---|---|
| [`backend/configuration.md`](backend/configuration.md) | 配置机制：三层覆盖、server/agent schema、env 映射、example 约定、部署面全景与不变量 |
| [`frontend/agent-onboarding.md`](frontend/agent-onboarding.md) | Agent onboarding 三态页 + 设备添加前置门控：配额经 `UserPublic.limits` 下发、pending/offline 语义、命令模板槽、安全上下文拦截 |
