# RFC: 为 hdsh 门禁退役本地文档标准

Status: implemented

[English](2026-10-03-retiring-the-local-documentation-standard.md) | 中文

## 问题

hdsh 接入落在了 wakewake 自有文档标准旁边，而不是取代它：两套门禁治理同一语料（`doc-check` 背后的七个 `scripts/verify_*.py` 门禁，加上七个 hdsh 托管的 prek hook）、两份预算清单钉住词数上限、两个配对语料相互重叠、两棵决策记录树（`.agents/wrfcs/` 与 `.agents/rfcs/`）割裂了历史。共存并不免费：本地 wrap 门禁把 HTML 注释槽位标记当作散文、在上游所有的 skills 上必红，而且每条文档规则都要同时满足两个语义不同的解析器。

## 决策

harness 完全取代本地标准。十一个本地门禁文件（`scripts/doc_sync.py`、`doclib.py`、七个 `verify_*.py`、两份文档 manifest）与 `doc-check` prek hook 移除；hdsh 托管块成为唯一的文档门禁集。AGENTS.md 词数上限从 `scripts/doc_budgets.manifest.json` 迁入 `.hdsh/docs.manifest.json`（`docBudgets`），由 `hdsh docs budgets` 持有；`docs/AGENTS.md` 改载 harness 文档标准，配对契约归 `docs/i18n/README.md`。五份 WRFC 记录按 harness 的 lifecycle/class 布局（`implemented/process`、`implemented/bug-fix`、`implemented/feature`、`proposed/architecture`）迁入 `.agents/rfcs/`，仅把 `# WRFC:` 标题前缀改写为 `# RFC:`，内容零改动；`.agents/wrfcs/` 树、其 README 对、以及被 harness 语料 skills 取代的 `writing-wrfcs` skill 一并移除。双语的 `specs/` 与 `devops/` 树经 manifest roots 留在配对语料内。

## 备选方案

两套门禁并行运行——否决：每份文档双重治理、要满足两个解析器、没有单一事实源。只逐项退役重叠的门禁——否决：每条规则的归属仍然含混并会漂移。为迁移记录重写内容以适配新词汇——否决：它们是当时的决策记录；纯移动（加上格式门禁要求的那一行标题）即保全了它们，记录内部陈述的过时由记录自身的生命周期处理，而不是靠重写历史。

## 后果

一套门禁、一个 manifest 属主：prek 承载本地卫生/镜像/提交 hook 加 hdsh 托管块，CI 的根 lint 作业包含 `hdsh` 组。配对语料失去 wrfcs README 对，保有二十九对。specs 索引规则改为评审把关（其门禁随套件移除）。config 模板注释仍以旧的 WRFC 字样指称决策记录；这些模板是 ask-first 文件，措辞留待它们因自身原因被编辑时再改。
