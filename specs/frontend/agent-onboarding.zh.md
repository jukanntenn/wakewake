# Agent Onboarding 与设备添加前置门控

[English](agent-onboarding.md) | 中文

面向普通用户的 agent 配对体验与设备添加的报错前置（错误在用户投入输入之前拦截并给出去向，而非提交后红字）。机制与不变量如下；实现位于 `agents/page.tsx`、`devices/page.tsx`、`DeviceForm.tsx`、`lib/agent-command.ts`、后端 `routes/auth.rs`。

## 真相源与不变量

- **配额唯一权威**是 `domain::MAX_DEVICES_PER_USER`（`backend/crates/server/src/domain/mod.rs`）。前端不得复制该数值；`UserPublic`（login / register / `GET /me` 共用构造器 `from_user`）携带 `limits.max_devices` 投影下发。服务端 `QUOTA_EXCEEDED` 校验保留，作为竞态兜底（多端同时添加）。持久化会话中 `limits` 缺失（旧会话）时跳过前置配额判断，回落到服务端兜底——降级可见于提交时错误文案，不静默造数。
- **agent 状态语义**：`public_key == null` ⇔ pending（从未配对）；有 key 且 SSE 在线 ⇔ online；有 key 且离线 ⇔ offline。设备添加的硬前提是**公钥存在**（前端用它加密 MAC），不是"在线"：
  - pending → 阻断添加，引导至 Agent 页；
  - offline → 放行添加，信息条告知"保存后待 Agent 上线自动同步"（与 `projection_status` 状态机一致）。
- **MAC 加密依赖安全上下文**（Web Crypto 在非 HTTPS、非 localhost 下不可用，见 `lib/crypto.ts`）。`window.isSecureContext` 为假时阻断添加并解释原因。
- **Agent 页命令模板槽**：启动命令由 `lib/agent-command.ts` 的三个生成器产出（`buildAgentInstallCommand` 一行安装 / `buildAgentDockerCommand` docker run 一键启动 / `buildAgentComposeYaml` docker compose，后者与 docker run 逐字段等价、末行以注释附带 `docker compose up -d`），`--server` 取 `window.location.origin`，配对码由调用方传入，终端卡头部 Linux / Docker 双 tab 切换（分发机制见 [`backend/agent-distribution.zh.md`](../backend/agent-distribution.zh.md)）。Docker tab 内为上下双块：compose（带「推荐」chip，独立复制整份 YAML）+ 一键启动（独立复制 run 命令）。命令与 README 实际渠道一字不差，不虚构。

## Agent 页（`/agents`）

三态页面，由 `useDefaultAgent()`（5s 轮询）驱动自动切换，无新增后端端点：

- **pending（引导态）**：心智模型图（lucide `Cloud` / `House` / `Monitor` 三节点 + hairline 连接线，服务器↔Agent 段断开高亮）→ 启动命令终端卡（hairline 头部 `sh` 标签 + `$` 前缀行，落地页同款视觉）→ 主按钮"复制命令"（整条命令一键复制，配对信息已内嵌）→ 等待行（脉冲，"页面将自动更新"）。提示文案说明 Agent 须运行在与被唤醒设备同一路由器下的常开机器（WoL 受限广播不跨路由器，RFC 919）。
- **online（完成态）**：模型图全连通；一行结论 + CTA「前往设备」（与设备页 onboarding 首尾衔接）；高级区折叠。
- **offline（修复态）**：结论"连接已断开" + 修复指引（检查那台机器是否在线，恢复后自动重连）+ 更换配对码入口（脱敏码无法直接用于重新配对，rotate 是获取完整码的唯一途径）。
- **高级折叠**（`<details>`，三态共用）：配对码（完整/脱敏）+ 复制 + 两段式轮换（armed 3s 模式不变）、Agent 公钥 PEM（pending 时显示"连接后可用"）、配置文件方式（`config.toml` 最小模板）。
- 文案走 next-intl 全部 8 个 locale，语气专业且友好。命令块不内嵌自然语言注释——唯一例外是 compose YAML：末行 `# 启动: docker compose up -d` 与 `network_mode: host` 行内约束注释是复制载荷的有意组成部分（粘贴即得自说明的合法 YAML）。

## 设备页（`/devices`）

**点击「添加设备」的前置状态机**（顺序固定，命中即拦截）：

```
agent.public_key == null        → 拦截弹窗：需要先连接 Agent [去连接 Agent →]
devices.length >= limits.max_devices → 拦截弹窗：设备已达上限（limits 缺失则跳过此判断）
!window.isSecureContext         → 拦截弹窗：需要 HTTPS 环境
其余                             → 打开表单
```

- 三个拦截共用一个轻量 Dialog（图标 + 标题 + 两行解释 + 动作）；按钮永远可点，点了永远得到解释。
- 标题旁常驻配额徽标 `{count}/{max}`（`limits` 缺失时不渲染）。
- **空状态 + pending**：设备列表为空且 agent 未配对时，空状态替换为 onboarding 引导卡（① 连接 Agent → ② 添加设备 → ③ 远程开机，当前步高亮，CTA 进 Agent 页）。
- **pending 且已有设备**（agent 被重置的罕见场景）：页首信息条引导重配对。
- **offline（已配对）**：页首信息条（离线 + 相对时间 + "可添加、待同步"说明 + 查看链接），不阻断。
- 设备卡片"Agent 离线"附注行带跳转链接。
- **MAC 失焦即校验**：`react-hook-form` `mode: 'onTouched'` + pattern 注册，格式错误在失焦时即呈现。
- **提交兜底保留**：服务端错误码映射（`QUOTA_EXCEEDED` / `AGENT_NOT_FOUND` / `RATE_LIMITED` / `SYNCING` / 字段级）不变；`connectorNotReady`（表单打开期间 agent 被重置的竞态）文案附内联链接至 `/agents`。编辑设备不受门控影响（MAC 锁定）。

## 验收

- 正常路径（pending 引导 → 配对 → online CTA → 添加设备）与全部拦截态（pending / 配额 / 非安全上下文 / offline 信息条）有组件级测试；`DeviceForm.test.tsx` 覆盖 MAC 失焦校验与兜底链接。
- 后端 `/me`、login、register 响应含 `limits.max_devices`（值来自 domain 常量）。
- i18n 键在 8 个 locale 全量存在（`en/zh/ja/ko/de/fr/es/pt`）。
- UI 以 Playwright 实机截图验证三态与门控。
