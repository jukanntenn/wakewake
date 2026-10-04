# RFC: Main-branch protection — PR-only landing with one approving review

Status: implemented

English | [中文](2026-10-04-main-branch-protection.zh.md)

## Problem

The harness adoption's out-of-git checklist step "Require one approval before merge" landed with administrators exempt: the classic branch protection on `main` required one approving review but left `enforce_admins` off, so the repository owner could push to `main` directly and could merge a pull request with zero reviews — the rules bound every future contributor except the one account that lands most changes. A protection the primary actor can bypass is convention, not enforcement: nothing mechanical stopped a direct push from skipping the commit-msg gate, the CI truth run, issue-policy validation, and the review the rule exists to force.

## Decision

`main` accepts changes only through pull requests, and every pull request — administrators not exempt — needs one approving review before it can merge. Direct pushes, force pushes, and deletion of `main` are rejected for all actors; the review requirement is the classic branch-protection rule set with `required_approving_review_count: 1` and `enforce_admins: true`, leaving merge methods, required status checks, and every other protection flag as they were. The pull-request path keeps its full gate stack: prek hooks and the `hdsh` group on every commit, CI on the pull request, the issue-policy workflow's kind/area validation, and the issue-lifecycle workflow's Start-Date initialization for referenced issues.

## Verification

Exercised from the owner account: a direct push of a throwaway commit to `main` is rejected by the server; merging the pull request that carried this record before any approving review is rejected; the same merge lands after one approving review from a second account. The issue-policy and issue-lifecycle workflows were observed on the same pull request, and a linked worktree was verified to install the `hdsh` hooks and merge driver worktree-locally.

## Alternatives considered

**Administrator exemption (the previous state).** Lost: the owner is the most frequent committer, so the rule would never bind the actor most likely to bypass it, and the protection could not be honestly verified from the owner account.

**A ruleset with the owner on a bypass list.** Lost: bypass dissolves the push and review rules at once for the same actor — the escape hatch reincarnates the previous state with more configuration.

**Require more than one approval.** Lost: the repository has one human owner; a higher bar names reviewers that do not exist and deadlocks every change.

**Convention and review discipline only.** Lost: unenforced review rules drift exactly like the documentation rules the harness gates exist to hold.

## Consequences

- Every change to `main` — hotfixes by the owner included — lands through a pull request with CI, policy validation, and one approving review.
- A pull-request author cannot approve their own pull request, so merges depend on a second account; if that account is unavailable, the owner's recourse is adjusting the protection itself, an audited admin action rather than a silent bypass.
- Force pushes and deletion of `main` stay blocked, so history on the trunk is append-only through merges.
