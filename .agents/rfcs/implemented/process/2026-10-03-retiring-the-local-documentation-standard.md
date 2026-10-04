# RFC: Retiring the local documentation standard for the hdsh gates

Status: implemented

English | [中文](2026-10-03-retiring-the-local-documentation-standard.zh.md)

## Problem

The hdsh adoption landed beside wakewake's own documentation standard instead of replacing it: two gate suites governed the same corpus (the seven `scripts/verify_*.py` gates behind `doc-check`, plus the seven hdsh-managed prek hooks), two budget manifests pinned ceilings, two pairing corpora overlapped, and two decision-record trees (`.agents/wrfcs/` beside `.agents/rfcs/`) split the history. The coexistence was not free: the local wrap gate parsed HTML-comment slot markers as prose and failed on upstream-owned skills, and every document rule had to satisfy two parsers with different semantics.

## Decision

The harness replaces the local standard entirely. The eleven local gate files (`scripts/doc_sync.py`, `doclib.py`, the seven `verify_*.py`, and the two doc manifests) and the `doc-check` prek hook are removed; the hdsh-managed block is the only documentation gate set. AGENTS.md word ceilings moved from `scripts/doc_budgets.manifest.json` into `.hdsh/docs.manifest.json` (`docBudgets`), so `hdsh docs budgets` owns them; `docs/AGENTS.md` now carries the harness documentation standard, and `docs/i18n/README.md` owns the pairing contract. The five WRFC records moved into `.agents/rfcs/` under the harness lifecycle/class layout (`implemented/process`, `implemented/bug-fix`, `implemented/feature`, `proposed/architecture`) with the `# WRFC:` title prefix rewritten to `# RFC:` and no other content change; the `.agents/wrfcs/` tree, its README pair, and the `writing-wrfcs` skill (superseded by the harness corpus skills) are gone. The bilingual `specs/` and `devops/` trees stay in the pairing corpus via manifest roots.

## Alternatives considered

Keep both gate suites running side by side — rejected: double governance of every document, two parsers to satisfy, and no single source of truth. Retire only the overlapping gates piecemeal — rejected: ownership of each rule would stay ambiguous and drift. Rewrite the migrated records' content for the new vocabulary — rejected: they are decision records of their time; a pure move (plus the one title-line the format gate requires) preserves them, and staleness inside a record is handled by the record's own lifecycle, not by re-authoring history.

## Consequences

One gate set, one manifest owner: prek carries the local hygiene/sync/commit hooks plus the hdsh-managed block, and CI's root lint job includes the `hdsh` group. The pairing corpus loses the wrfcs README pair and keeps twenty-nine pairs. The specs index rule is now review-enforced (its gate is gone with the suite). Config-template comments still name decision records by the old WRFC term; those templates are ask-first files, so the wording stays until they are edited for their own reasons.
