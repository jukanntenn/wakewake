# Coding principles

English | [中文](PRINCIPLES.zh.md)

Behavioral constraints for the agent. Each is a rule the agent gets wrong without being told. Production safety and data integrity outrank every principle here — including the license to redesign from scratch; against a mere default or style rule, the principle wins.

## Ground every conclusion in fact

Library facts, APIs, and protocols must be read from source or docs before you act on them — training data is a blind spot, not a source. Verify every conclusion on the ground: `file:line` for logic, a headless browser (Playwright) for UI, read-only commands in prod, with means the model can actually perform (no screenshot analysis). Pure algorithm or syntax knowledge may use training knowledge.

The CI 0s-failure incident is the shape of getting this wrong: `check-yaml` "validated" the workflows, but a misplaced `paths-ignore` is legal YAML and an invalid workflow — verifying with a means blind to that failure class green-lit four broken workflows until `actionlint` (workflow-aware) was added.

## Defer to community convention

When a convention or best practice is uncertain, ask "what is the community/official convention?" and verify against authoritative open-source source, not training memory (e.g. whether `format`/`lint`/`check` are the prek group names, or SemVer prerelease syntax `v0.1.3-rc.1` — never `v0.1.3rc1`).

Distinct from *Ground every conclusion in fact*: that one governs facts about a library you are integrating; this one governs convention and best-practice decisions.

## Converge before you implement

A spec or plan must be self-contained, complete, and unambiguous — an executor with no taste can land it mechanically, with no room to improvise. Resolve every open point before implementing; do not start on the strength of a half-settled plan.

## Fix the root cause, not the symptom

The solution you choose must be the most natural and optimal — not a patch over the symptom, and not one trapped by the existing implementation. You may shed all legacy and start from zero when the root fix requires it.

When local-green/CI-red gate drift was the risk, the fix was not to police the workflows but to converge every gate on prek and have CI re-run the same hooks — drift became structurally impossible instead of watched for.

## Design from first principles

Derive a design from the business essence; every premise is breakable; an elegant scheme beats an inherited one. Distinct from *Fix the root cause, not the symptom*: that one is how you *fix* a problem (root, not patch); this one is how you *design* a system (re-derive, question assumptions).

## Single source of truth

Each category of information — config, i18n, gate commands, agent instructions — has exactly one authoritative source; every other copy is generated. Agent instructions live in `AGENTS.md`; `CLAUDE.md` is a direction-free mirror (`scripts/agentlib.py` + the `agent-instructions-sync` gate keep the pair equal, whichever side was edited). Gate commands live once in the three `prek.toml`s; CI re-invokes the same hooks rather than restating commands, and the editor hooks delegate to prek instead of restating formatters. The frontend renders; it does not decide.

## Naming is part of the API

A name is an API surface. If a name does not fit its business meaning, do not force it — brainstorm candidates and let the user choose, to prevent semantic drift.

## Degrade gracefully, never silently

A failure must be handled and observably recorded, and must not block downstream work — but a silent failure is always wrong. `scripts/backend_tests.py` is the shape: no PostgreSQL reachable → fall back to `cargo test --lib`, with a loud warning that integration tests only ran where a PG exists — the degradation is visible, never swallowed.

## Minimal mock, maximal real

Mock only the request boundary (Bemfa, mailer), never the whole service. Integration tests run against a real PostgreSQL; the SSE Hub and internal logic are never mocked; e2e drives the full container stack — locally and in CI the same suite.
