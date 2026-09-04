# WRFC: Agent harness——wrfcs、分层指令、skills 源与文档门控

Status: implemented

[English](2026-08-22-agent-harness-mechanism.md) | 中文

## Problem

wakewake 没有决策理由的归宿。已发布设计的“为什么”散落在 PRINCIPLES.md 的事故叙事和 `.local/specs/` 下未入库的遗留文档树里；等待评审的提案更没有持久的落点，于是每个非平凡变更都要重新推导事实、重新争论已定的问题。围绕这个缺口还有五个常驻缺陷：指令语料是一个扁平文件——根 AGENTS.md 1439 词外加字节拷贝 CLAUDE.md，每个会话无论任务为何都要加载全部区域的命令与约定；拷贝由方向性同步维护（scripts/sync_agents.py 把 AGENTS.md 复制到 CLAUDE.md），在 CLAUDE.md 一侧做的编辑会被旧内容静默覆盖——正是镜像机制绝不能犯的“旧覆盖新”失效；skills 在 `.zcode/skills/`（四个）与 `.claude/skills/`（三个）之间手动双拷贝、无任何机制，且已经漂移（`iterating` 只存在于 ZCode 一侧）；Markdown 语料混用语言且无配对规则（AGENTS.md 与 PRINCIPLES.md 是英文，`specs/` 是纯中文，README 对使用 `README_zh.md` 命名），也没有门控——链接无声腐烂、spec 不入索引就入库（specs/backend/agent-distribution.md）、历史叙事无处安放；agent hooks 重复定义门禁命令——`.claude/hooks/format.py` 与 `lint_stop.py` 硬编码 rustfmt/prettier/clippy 调用，与三份 prek 配置重复，正是 PRINCIPLES #6 点名的单一真相源违例，而 `.zcode/` 干脆没有任何 hooks。

## Decision

本机制的适配源自两份完整读过的参照：deepseek-harness 参考项目，以及 markpost 对它的适配（其决策史见该仓库的 `.agents/mrfcs/`，正是本记录所循先例）。是适配而非照搬——下面每一块都写明 wakewake 保留什么、改什么、拒绝什么。

**wrfcs 树。** 记录存放于 `.agents/wrfcs/{proposed,implemented,rejected}/yyyy-mm-dd-topic.md`，日期为主题首次提出之日；树本身即清单——浏览或 grep 仓库即可，不维护索引文件。树根有两份职责正交的文档：[AGENTS.md](../AGENTS.md) 只放常驻命令（动笔前查重、以新记录取代而非重写、双语对同步），[README.zh.md](../README.zh.md) 是唯一规范契约；README 以 `README.md` + `README.zh.md` 配对，常驻命令文件按 agent 指令豁免规则保持单文件。文件格式为固定头部块、必须与目录一致的 `Status:` 行、以 `## Problem` 开篇（须脱离方案独立成立）的正文，以及按生命周期分化的续篇——implemented 用现在时的 `## Decision` / `## Alternatives considered` / `## Consequences`（方案话术标题被门控拒绝），proposed 用 `## Proposal` / `## Alternatives considered` / `## Acceptance criteria` / `## Risks`，rejected 冻结提案期内容、判决写在 `Status:` 行上。触发规则：每个非平凡变更——行为、架构、跨文件契约、工具链、测试策略、盘上或线上格式——在同一变更中新增或更新至少一份记录，动笔前先 grep 树中已有归属。本记录是第一份 wrfc，并在机制自身上演练了它：先落在 `proposed/` 评审，再由落地机制的这一批改写为现在的 implemented 形态。

**文档标准。** 唯一家园：[docs/AGENTS.md](../../../docs/AGENTS.md)（wakewake 此前没有 `docs/`）。它承载层级表——README、根 AGENTS.md、子树 AGENTS.md、PRINCIPLES.md、`specs/`、`docs/`、wrfcs、skills——为每类事实指定唯一归宿（“在他处只链接，不复述”），以及下述七条写作规则，每条规则点名执行它的门控。`specs/README.md` 保持 specs 索引职责并配对；markpost 用 `index.md` 的位置 wakewake 沿用 README.md 命名，因为代码托管平台会免费渲染目录下的 README.md。`.local/` 下的运维手册保持不入库、不在范围内。

**双语配对，覆盖全部语料。** 每份文档以 `foo.md` 与 `foo.zh.md` 并存，双方等权威：任一侧都可先写、先改，被编辑的一侧是该次变更的源，另一侧在同一变更中以最小补丁跟随——绝不做整体重译；两侧实质冲突时改错的那侧，没有哪种语言默认胜出。`README_zh.md` 现为 `README.zh.md`。范围是入库的文档语料——README 对、PRINCIPLES.md、`specs/`（含索引）、`docs/` 内容、`.agents/wrfcs/**`、`devops/README.md`、`frontend/README.md`、`backend/crates/agent/README.md`——豁免记录在只含豁免的 manifest（`scripts/doc_languages.manifest.json`）：agent 指令（所有 AGENTS.md/CLAUDE.md/SKILL.md 及 skills 镜像）、`.zcode/`、生成文件。配对契约：每侧头部有语言切换链接（裸文件名）、链接语言域（`.zh.md` 页面链接 `.zh.md` 目标，反之亦然，切换链接除外）、结构对齐（标题层级序列一致、围栏代码块含注释字节一致——示例不翻译，故中文主写的 specs 其含中文注释的代码块两侧原样保留）、机器标记与节标题两侧都保持英文、英文侧散文除 manifest 豁免外不得出现 CJK。语料以一批十一个对完成规范化；暂存态门控容忍尚未写出的孪生链接目标，CI 的严格全量跑拥有终态。

**分层指令与词数预算。** 根文件保留每个会话都需要的——身份、技术栈、结构图、跨区命令索引（`dev.py`、文档门控）、git 工作流、跨区边界、指向 WRFC 与文档家园的单行指针、说明镜像契约的“编辑本文件”一节——710 词，低于 800 上限。三份子树文件承载工作区专属命令：[backend/AGENTS.md](../../../backend/AGENTS.md)（cargo 命令、迁移规则、Rust 风格、测试契约）371/500，[frontend/AGENTS.md](../../../frontend/AGENTS.md)（pnpm 命令、React/TypeScript 风格、静态导出约束、Vitest）166/300，[e2e/AGENTS.md](../../../e2e/AGENTS.md) 125/150；子树文件补充根文件、绝不复述它。预算是 `scripts/doc_budgets.manifest.json` 中的 `wc -w` 整文件上限；被预算的文件是纯英文的 agent 指令，因此 `wc -w` 足够、无需中文计数；被预算的文件缺失即失败，改名无法让预算成为孤儿。docs/AGENTS.md 另有 600 词上限（实际 454）。

**方向无关镜像。** [scripts/agentlib.py](../../../scripts/agentlib.py)，移植自 markpost：每对 AGENTS.md ↔ CLAUDE.md（根、backend、frontend、e2e）以 git HEAD 判定方向——恰好一侧相对 HEAD 有变更，则该侧覆盖旧的一侧并由门控暂存；内容相等即通过；两侧都变更且不一致是工具拒绝猜测的冲突，报错时列出两侧、修复命令与人工调和步骤；一侧不在 HEAD 中则自举其孪生。mtime 作为方向信号被否决，因为 clone 与 checkout 会重置它。`scripts/check_agent_instructions.py` 是可自愈的 prek pre-commit 门控，`scripts/sync_agent_instructions.py` 是手动修复器，方向性的 `scripts/sync_agents.py` 及其 `agents-sync` 钩子已移除。移植中还加固了演练暴露的一处 markpost 缺陷：`sync_skills` 先删镜像后查源，源缺失时会毁掉两个镜像且无从重建——现在任何 `rmtree` 之前先守卫源。

**Skills。** `.agents/skills/` 是源；`.zcode/skills/` 与 `.claude/skills/` 是字节镜像，由同步脚本整体重建、由门控校验——markpost 把 `.agents/` 的这一提升记为未来工作，而 wakewake 在本次变更里就创建了 `.agents/`，所以直接到位（原先的漂移——`.claude` 缺 `iterating`——随之消失：两个镜像现在都带全部五个技能）。新技能：`writing-wrfcs`（何时欠一份记录、格式、生命周期移动、配对义务）。`commit` 技能承载配对捆绑规则——README 对、wrfc 对、spec 对，作为一个逻辑单元暂存，绝不单侧——以及新的镜像契约。`iterating` 与 `shipping` 未变；独立的 `doc-standards` 技能保持信号触发：每条规则已有门控和根文件指针。

**七项门控，一个编排器。** 纯 stdlib 脚本共享 [scripts/doclib.py](../../../scripts/doclib.py)（保形掩码使诊断仍指向真实行号；GitHub 兼容的标题 slug），由 [scripts/doc_sync.py](../../../scripts/doc_sync.py) 按序编排，每个都可独立运行、可按给定文件收窄：`verify_md_links.py`（相对目标存在；`#fragment` 能在目标标题中解析）、`verify_md_wrap.py`（散文段落一物理行）、`verify_md_current.py`（README/docs/specs 禁历史叙事——历史归 wrfcs）、`verify_specs_index.py`（specs 索引双向完整、每对一行按词干匹配）、`verify_wrfc_format.py`（骨架、Status 与目录一致、生命周期标题、`.zh.md` 孪生在场）、`verify_doc_pairs.py`（双向完整性、切换链接、链接语言域、结构对齐、英文侧纯净度、manifest 自校验）、`verify_doc_budgets.py`。根 [prek.toml](../../../prek.toml) 在 lint 组、pre-commit 阶段承载 `doc-check`，作用于暂存 Markdown，排除 `.zcode/`——只查暂存集，因为 prek 在钩子运行期间会 stash 未暂存变更；全量语料经同一按组调用在 CI 跑。

**Hooks 委托 prek。** PostToolUse 对刚编辑的文件运行 `prek run --group format --files <file>`；Stop 带每轮一次守卫运行 `prek run --group lint --all-files`。format 组现在是可变更层——backend `cargo fmt`、frontend `prettier --write`、根上为 Python 加 `ruff format`——只读检查（`cargo fmt --check`、`prettier --check`）以 `fmt-check`/`format-check` 之名移入 lint 组，且在各自项目配置中定义在修复器**之前**，使 CI 的按组调用先在干净树上检查、再让任何修复器动手；CI 覆盖面不变（漂移仍然失败）。三个薄适配器手工维护，仅在 payload 形态上不同：`.claude/hooks/`（snake_case payload），`.zcode/hooks/`（camelCase payload；因 ZCode v2.1.0 剥离 workspace 级 hooks 而在用户级注册，一次性注册步骤记录在 `.zcode/README.md`），以及 `.codex/hooks/`（解析 apply_patch 路径）。旧的 `format.py`/`lint_stop.py` 与 `scripts/sync_hooks.py` 已删除。CI 的 lint.yml 从 workflow 级 `paths-ignore` 中移除了 `**/*.md`、`docs/**`、`specs/**`，文档变更现在会触发根 lint 作业（doc-check）；backend/frontend 作业仍在 dorny/paths-filter 之后。

## Alternatives considered

**整体移植参考项目的机制**——TypeScript 门控链、带 `needs`/`after` 依赖边的调度器、每份记录的 `.md` + `.zh.md` + `.i18n.yaml` 三元组、密封归档：约 5300 行工具服务约 1150 个配对。落选：wakewake 语料小一个数量级，仓库没有根级 Node 工具链，Python stdlib 套件已覆盖采纳的每条规则；机制只按触发信号逐块引入，这是两份先例共同记录的纪律。

**只有规则、没有门控。** 落选：无纪律的漂移正是门控存在的理由；标准里每条规则都点名执行它的门控，没有机器的规则只会再造它所禁止的失效。

**locale 目录（`en/` 与 `zh/` 子树）。** 落选：并肩配对让孪生留在同一个 diff 与同一次评审里，链接按后缀即可切换语言；目录会把每页拆离自己的维护，把链接图变成要分别看守的两棵树。

**英文为主、中文为翻译。** 落选：维护者的工作语言是中文，且 `specs/` 原本就是中文原生；主次规则会压低更可能先写的一侧，并把现有中文页重新归档为并不存在的翻译的派生物。

**保留 `README_zh.md` 命名。** 落选：`.zh.md` 是 wrfc 树、格式门控文件名正则与所有新配对共享的约定；一个概念两种拼法本身就是漂移。

**存量单语文件老帐豁免。** 落选：为一次性有界成本——十一个对，最大的 `specs/backend/configuration.md` 约 300 行——换来永久残缺一半语言的两类语料。

**wrap 与预算门控按信号缓引。** 预算一项落选：1439 词的根文件就是已经响过的膨胀信号；wrap 搭同一 stdlib 套件的便车，一次性重排本来就要由规范化承担。

**保留方向性同步。** 落选：方向性是缺陷本身而非调参选择——被定为次要的那一侧会被修复器静默回滚，而那一侧恰是其工具加载它、其 agent 最可能编辑它的一侧。

**符号链接镜像。** 落选：git 把符号链接存为持有目标路径的 mode-120000 blob；未开 `core.symlinks` 的 Windows checkout 会把 CLAUDE.md 变成内容为九个字节的普通文件，而没有任何东西把贡献者约束在类 Unix 环境上。

**以 mtime 判定方向。** 落选：clone 与 checkout 把 mtime 重置为当前时刻，两侧同样“新”；对 HEAD 的内容比对是确定性的，不依赖任何文件系统状态。

**以 `.zcode/skills/`（或 `.claude/skills/`）为 skills 源。** 落选：`.agents/` 是 agent 资源的生态加载路径，而本次变更本来就要创建它；现在提升避免了 markpost 只能记为未来工作的那次迁移，且无论哪边权威，两个工具目录都需要字节拷贝。

**把文档标准写进 `specs/README.md`。** 落选：索引与标准是两份职责；合并会把一页索引变成常驻命令页，而索引必须保持为 spec 作者可以机械更新的表。

**按 markpost 四份记录的方式每个子机制一份 wrfc。** 落选：那四份是在数天里各自评审、各自落地的独立变更集；本机制作为一个批次落地，就得到一份记录，其小节随批次走深。

## Consequences

七项门控在规范化后的语料上双模式全绿——暂存（pre-commit）与全量（等价 CI 的无参 `doc_sync.py`）：40 个文件的相对链接与锚点全部解析、40 个无硬换行段落、17 个纯现态行文、3 个 spec 对在双索引中各有一行、11 个双语对通过完整性/切换链接/链接语言域/结构/纯净度校验、5 个被预算文件在上限之内、本记录对符合 wrfc 格式。prek 三组端到端演练通过：根 lint 组全量跑 `doc-check` 与 `actionlint` 全绿；check 组跑 builtin 卫生钩子加 `agent-instructions-sync` 全绿；format 组首跑把三个存量 Python 脚本纳入 ruff 格式化（此前只有被 AI 编辑过的文件才会被格式化）——机械重排，已核实无内容变化。`README.zh.md` 完成重命名且所有入站链接更新；skills 镜像与 `.agents/skills/` 字节一致；规范化顺带修正了门控点名的问题——`devops/README.md` 与一份 spec 中的三处历史叙事短语、agent README 的 `../../releases` 断链、README 指向未入库 spec 目录的引用、以及一处指向 `.local` 专属 spec 的链接（它本会让 CI 的链接门控变红）。镜像演练澄清了两个现已写入契约的事实：方向检测读 git HEAD，变更集未提交期间对另一侧的任何编辑都会登记为工具拒绝猜测的双侧冲突（自愈路径在批次提交后生效）；移植关闭了 markpost 在 `sync_skills` 上“先删后查源”的缺陷。接受的代价：结构对齐证明的是形状而非含义——粗糙的孪生也能全绿，语义由评审承担；只查暂存集的 pre-commit 门控意味着全量真相在 CI；预算上限会诱发上调拉锯，靠余量与文档化的上调路径吸收；ZCode 剥离 workspace hooks 使用户级注册成为每台机器的手工步骤（`.zcode/README.md` 是缓解）；backend/frontend prek.toml 中 fmt-check 定义在 fmt 之前的定义序对 CI 按组调用是承重结构，后续编辑必须保留；移植来的 `used to` 叙事短语同样匹配被动语态，故语料以主动措辞行文（"Email links are built from it"），这一约束由未来的英文散文继承。
