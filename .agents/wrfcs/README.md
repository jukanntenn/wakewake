# WakeWake Request for Comments (WRFC)

English | [中文](README.zh.md)

WRFCs are wakewake's RFCs: durable proposals and decision records — the _why_, _what we gave up_, and the parts code and specs cannot carry. Specs describe current state; WRFCs explain why that state is what it is.

## Layout and naming

Every WRFC lives at `.agents/wrfcs/{lifecycle}/yyyy-mm-dd-topic-title.md`. The date is when the topic was first proposed (per git history). The lifecycle tree is the inventory — browse it or grep the repository; there is no index file to maintain.

- **`proposed/`** — proposals reviewed before implementation. Not yet built, or only partly built.
- **`implemented/`** — the decision shipped. The file records what was decided and what was rejected, in the present tense. When code later renames a file or changes a default, update the WRFC's facts (paths, names, structure) in the same change — but never edit it into a different decision; supersede it with a new WRFC and cross-link both.
- **`rejected/`** — the proposal was considered and declined. Keep one only while its rationale prevents a tempting mistake; otherwise delete it.

Cross-references between WRFCs use relative Markdown links, never bare prose, so [`verify_md_links`](../../scripts/verify_md_links.py) can check them and they survive moves between folders.

## When to write one

Every non-trivial change adds or updates at least one WRFC in the same change. A change is non-trivial when it alters behavior, architecture, a contract shared across files, tooling, testing strategy, an on-disk or wire format, or anything a maintainer may reasonably revisit. Purely mechanical or local edits are exempt. Updating the WRFC that already owns the decision satisfies the rule — do not create a duplicate; grep `.agents/wrfcs/` for the topic first.

## The file format

The header block is exactly:

```markdown
# WRFC: <title>

Status: <status>
```

The `Status:` value must agree with the folder and takes one of three forms: `proposed`, `implemented`, or `rejected — <why, in one line>` (the rejection reason is the fact readers come for). The body opens with `## Problem`, written to stand without the solution.

`implemented/` continues `## Decision` (present tense, what shipped) … `## Alternatives considered` … `## Consequences`. Proposal-era headings — `## Proposal`, `## Plan`, `## Migration plan`, `## Acceptance criteria` — are rejected here by the format gate.

`proposed/` continues `## Proposal` … `## Alternatives considered` … `## Acceptance criteria` … `## Risks`. A proposal may speak in the future tense while the work is unbuilt.

`rejected/` keeps whatever proposal-time sections it had, frozen; the verdict lives on the `Status:` line.

Every record is a bilingual pair: the English original with a `.zh.md` twin beside it — same skeleton, machine tokens and section headings in English — and the pair updates together ([the bilingual-pairs rule](../../docs/AGENTS.md)). [`verify_wrfc_format`](../../scripts/verify_wrfc_format.py) checks both skeletons.
