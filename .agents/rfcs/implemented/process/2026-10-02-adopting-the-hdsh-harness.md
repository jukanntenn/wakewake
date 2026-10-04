# RFC: Adopting the hdsh harness

Status: implemented

English | [中文](2026-10-02-adopting-the-hdsh-harness.zh.md)

## Problem

Before this change, jukanntenn/wakewake carried none of the harness mechanisms: no prek governance gates, no issue policy or lifecycle automation, no bilingual documentation pairing, and no RFC, skill, or documentation-standard scaffolding. Repository conventions lived in reviewer memory instead of mechanically executed checks, and every adoption question had to be answered from the harness repository's own documentation.

## Decision

This repository adopts the hdsh harness at ref `d1634158cc7aad1dec8ad16e0c3f2b0edfb4cffa` through `hdsh adopt apply`: the prek gate set pinned to that ref, the composite-action issue policy and lifecycle workflows, the bilingual documentation pairing corpus with its merge driver, the RFC mechanism, the workflow skills, and the documentation standard. Issue management runs against the `wakewake` project described by `.github/issue-management/config.json`; the installed file inventory and digests are recorded in `.hdsh/adopt.manifest.json`, and upgrades rerun `hdsh adopt apply` under the newer ref.

## Alternatives considered

**Staying without gates.** Zero setup cost, but conventions stay unenforced and drift is caught only in review.

**Hand-copying the harness files.** A one-time copy works until the harness improves; every upgrade becomes a manual re-diff per file, which is the failure mode `hdsh adopt` and its manifest exist to remove.

## Consequences

- The repository adopts the harness tree conventions: every README, `docs/**`, and `.agents/rfcs/**` join the bilingual pairing corpus.
- Generated files are upstream-owned; `hdsh adopt verify` reports drift and remaining `TODO(adopt)` placeholders, and changes to generated files redirect upstream instead of forking.
- The adopted skills were localized once where they named the upstream repository: `documenting`, `editing-prose`, `finding-simplifications`, and `translating-docs` say wakewake with their `.hdsh/adopt.manifest.json` digests re-anchored, and the slot-template skills (`archiving-rfcs`, `reviewing`) carry the same renames in consumer-owned prose outside their slots. A later `hdsh adopt apply` takes the upstream bytes, so durable wording fixes still redirect upstream.
- The out-of-git state — labels, Project fields and statuses, secrets and variables, branch protection — was configured separately following the harness ADOPT.md checklist.
