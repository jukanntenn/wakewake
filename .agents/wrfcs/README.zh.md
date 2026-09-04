# WakeWake Request for Comments (WRFC)

[English](README.md) | 中文

WRFC 是 wakewake 的 RFC：持久的提案与决策记录——_为什么_、_放弃了什么_、以及代码和规范承载不了的部分。规范描述现态；WRFC 解释现态为何如此。

## Layout and naming

每份 WRFC 存放于 `.agents/wrfcs/{lifecycle}/yyyy-mm-dd-topic-title.md`。日期为主题首次提出之日（依 git 历史）。生命周期树即清单——浏览或 grep 仓库即可，不维护索引文件。

- **`proposed/`**——实施前需评审的提案。尚未构建，或只部分构建。
- **`implemented/`**——决策已落地。文件以现在时记录决定了什么、拒绝了什么。当后续代码重命名文件或改默认值时，同一变更中更新该 WRFC 的事实（路径、名称、结构）——但绝不把它改写成另一个决策；用新 WRFC 取代并互链。
- **`rejected/`**——提案被考虑后否决。仅当其理由还能防止一个诱人的错误时保留；否则删除。

WRFC 之间的交叉引用使用相对 Markdown 链接，绝不使用裸文字，这样 [`verify_md_links`](../../scripts/verify_md_links.py) 才能校验，且在目录间移动后依然有效。

## When to write one

每个非平凡变更在同一变更中新增或更新至少一份 WRFC。变更非平凡指：改变行为、架构、跨文件契约、工具链、测试策略、盘上或线上格式，或维护者可能合理重审的任何东西。纯机械或局部编辑豁免。更新已拥有该决策的 WRFC 即满足规则——不要造重复；先 grep `.agents/wrfcs/` 查主题。

## The file format

头部块固定为：

```markdown
# WRFC: <title>

Status: <status>
```

`Status:` 值必须与目录一致，取三种形式之一：`proposed`、`implemented`、`rejected — <一行理由>`（拒绝理由是读者要找的事实）。正文以 `## Problem` 开篇，须脱离方案独立成立。

`implemented/` 续以 `## Decision`（现在时，落地了什么）……`## Alternatives considered`……`## Consequences`。提案期标题——`## Proposal`、`## Plan`、`## Migration plan`、`## Acceptance criteria`——在这里被格式门控拒绝。

`proposed/` 续以 `## Proposal`……`## Alternatives considered`……`## Acceptance criteria`……`## Risks`。工作未建时提案可用将来时。

`rejected/` 冻结保留提案期的任何章节；判决写在 `Status:` 行上。

每份记录都是双语对：英文原作旁附 `.zh.md` 孪生——同骨架、机器标记与节标题保持英文——两侧同变更一起更新（见[双语配对规则](../../docs/AGENTS.md)）。[`verify_wrfc_format`](../../scripts/verify_wrfc_format.py) 校验两侧骨架。
