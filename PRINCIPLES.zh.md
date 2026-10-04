# Coding principles

[English](PRINCIPLES.md) | 中文

对 agent 的行为约束。每一条都是“不被告知就会做错”的规则。生产安全与数据完整性凌驾于本文所有原则之上——包括从零重写的授权；与单纯的默认值或风格规则冲突时，原则胜出。

## Ground every conclusion in fact

库的事实、API 与协议必须在动手前从源码或文档读取——训练数据是盲区，不是来源。每个结论都要在实地验证：逻辑用 `file:line`，UI 用无头浏览器（Playwright），生产环境用只读命令，且手段须是模型真正能执行的（不做截图分析）。纯算法或语法则可用训练知识。

CI 0 秒失败事故是做错的形状：`check-yaml`“验证过”了 workflows，但放错位置的 `paths-ignore` 是合法 YAML 却是非法 workflow——用对该失效类别失明的手段去验证，给四个坏 workflow 亮了绿灯，直到补上 workflow 感知的 `actionlint`。

## Defer to community convention

约定或最佳实践不确定时，先问“社区/官方约定是什么”，再对照权威开源源码验证，而不是依赖训练记忆（例如 prek 的组名是不是 `format`/`lint`/`check`，SemVer 预发布语法 `v0.1.3-rc.1`——绝不是 `v0.1.3rc1`）。

与 *Ground every conclusion in fact* 的区别：那条管“要集成的库的事实”，这条管“约定与最佳实践的取舍”。

## Converge before you implement

spec 或计划必须自含、完整、无歧义——一个没有品味的执行者也能机械落地，没有即兴发挥的空间。实现前解决每一个未定点；不要靠一个半生不熟的计划开工。

## Fix the root cause, not the symptom

选定的解法必须是最自然、最优的——不是在症状上打补丁，也不是被既有实现困住。当根因修复需要时，可以甩掉全部遗留、从零开始。

当“本地绿/CI 红”的门禁漂移成为风险时，修复不是派人盯 workflow，而是把所有门禁收敛到 prek、让 CI 重跑同一批 hooks——漂移从“需要提防”变成“结构上不可能”。

## Design from first principles

设计从业务本质推导；每个前提都可以打破；优雅的方案胜过继承来的方案。与 *Fix the root cause, not the symptom* 的区别：那条讲怎么*修*问题（根因，非补丁），这条讲怎么*设计*系统（重推导，质疑假设）。

## Single source of truth

每一类信息——配置、i18n、门禁命令、agent 指令——都只有唯一权威源；其余副本都是生成的。Agent 指令活在 `AGENTS.md`；`CLAUDE.md` 是方向无关镜像（`scripts/agentlib.py` + `agent-instructions-sync` 门控保持两侧相等，无论编辑的是哪一侧）。门禁命令只活在三份 `prek.toml` 里；CI 重跑同一批 hooks 而非复述命令，编辑器 hooks 也委托 prek 而不复述格式化器。前端只渲染，不做决定。

## Naming is part of the API

名字就是 API 面。名字不符合业务含义时不要硬用——头脑风暴候选名，让用户选择，防止语义漂移。

## Degrade gracefully, never silently

失败必须被处理并被可观测地记录，且不阻塞下游——但静默失败永远是错的。`scripts/backend_tests.py` 是形状：找不到 PostgreSQL → 回退 `cargo test --lib`，并大声警告集成测试只在有 PG 的地方跑了——降级可见，绝不吞掉。

## Minimal mock, maximal real

只 mock 请求边界（巴法云、邮件），绝不 mock 整个服务。集成测试跑真实 PostgreSQL；SSE Hub 与内部逻辑从不 mock；e2e 驱动完整容器栈——本地与 CI 同一套件。
