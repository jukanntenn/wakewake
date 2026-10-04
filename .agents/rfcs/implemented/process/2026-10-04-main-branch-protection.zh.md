# RFC: main 分支保护——仅经 PR 落地且需一枚批准评审

Status: implemented

[English](2026-10-04-main-branch-protection.md) | 中文

## 问题

harness 采纳的 git 外清单步骤"Require one approval before merge"落地时豁免了管理员：`main` 上的经典分支保护要求一枚批准评审，但 `enforce_admins` 处于关闭状态，仓库所有者既可以直接向 `main` 推送，也可以在零评审下合并 pull request——规则约束每一个未来的贡献者，唯独不约束落地绝大多数变更的那个账号。主要执行者可以绕过的保护只是惯例而非强制：没有任何机制阻止一次直接推送绕过 commit-msg 门禁、CI 真源运行、issue-policy 校验，以及该规则存在的目的——评审本身。

## 决策

`main` 只接受经 pull request 的变更，且每个 pull request——管理员不豁免——必须持有一枚批准评审才能合并。直接推送、force push 与删除 `main` 对所有角色一律拒绝；评审要求即经典分支保护规则集上的 `required_approving_review_count: 1` 加 `enforce_admins: true`，合并方式、必需状态检查及其余保护旗标维持原状。pull request 路径保留完整门禁栈：每次提交的 prek 钩子与 `hdsh` 组、PR 上的 CI、issue-policy workflow 的 kind/area 校验，以及 issue-lifecycle workflow 为被引用 issue 初始化 Start Date。

## 验证

从所有者账号实际演练：向 `main` 直接推送一个临时提交被服务端拒绝；携带本记录的 pull request 在获得任何批准评审前合并被拒绝；同一合并在第二账号给出的一枚批准后落地。同一个 pull request 上观察到 issue-policy 与 issue-lifecycle 两个 workflow 的运行，并验证了关联 worktree 以 worktree 本地方式安装 `hdsh` 钩子与合并驱动。

## 备选方案

**管理员豁免（此前状态）。** 落选：所有者是最高频的提交者，规则永远约束不到最可能绕过它的执行者，保护也无法从所有者账号得到诚实的验证。

**把所有者放进 bypass 名单的 ruleset。** 落选：bypass 会同时溶解推送与评审两条规则——逃生门以更多配置 reincarnate 了此前状态。

**要求多于一枚批准。** 落选：仓库只有一位人类所有者，更高的门槛指定了并不存在的评审者，每个变更都会死锁。

**仅靠约定与评审纪律。** 落选：没有强制力的评审规则，漂移方式与 harness 文档门禁所要守住的文档规则一模一样。

## 后果

- `main` 的每个变更——包括所有者的热修——都经 pull request 落地，带 CI、策略校验与一枚批准评审。
- pull request 作者不能批准自己的 PR，因此合并依赖第二账号；该账号不可用时，所有者的救济是调整保护本身——一个留痕的管理动作，而非静默绕过。
- `main` 的 force push 与删除持续被拒，主干历史只能经合并追加。
