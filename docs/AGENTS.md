# AGENTS.md — The documentation standard

English prose only in this file (agent instructions are exempt from pairing). The tiers, the rules, and the gate behind each rule; see also [`.agents/wrfcs/AGENTS.md`](../.agents/wrfcs/AGENTS.md) for the WRFC standing orders.

## Tiers: one fact, one home

| Tier | Owns |
|---|---|
| `README.md` / `README.zh.md` | Product documentation: what it is, quick start, deployment |
| Root `AGENTS.md` | Standing orders every session needs, 1–3 lines per rule, linking the rule's home |
| `backend/AGENTS.md`, `frontend/AGENTS.md`, `e2e/AGENTS.md` | Subtree-specific orders; never repeat the root |
| `PRINCIPLES.md` | Behavioral principles, each grounded in a real incident |
| `specs/` | Current-state design reference, indexed by `specs/README.md` |
| `docs/` | This standard |
| `.agents/wrfcs/` | Proposals and decision records (the why, the alternatives) |
| `.agents/skills/` | Agent workflows (source; mirrored to `.zcode/skills/` and `.claude/skills/`) |
| `devops/README.md` | Build/deploy operations |

Elsewhere, link; never restate. Agent instructions (`AGENTS.md`/`CLAUDE.md`/`SKILL.md`) are exempt from bilingual pairing; everything else in the corpus pairs.

## Rules and gates

1. **Current-state prose.** README/docs/specs narrate what is, not what changed; history lives in a WRFC. Gate: `verify_md_current.py`.
2. **Machine-checkable links.** Relative links and `#fragment` anchors must resolve. Gate: `verify_md_links.py`.
3. **One physical line per paragraph.** Write one line per paragraph; let the editor soft-wrap. Gate: `verify_md_wrap.py`.
4. **Specs index completeness.** Every spec pair has exactly one row in `specs/README.md` and `specs/README.zh.md`, both directions. Gate: `verify_specs_index.py`.
5. **WRFC format.** Skeleton, Status-folder agreement, lifecycle headings, the bilingual twin. Gate: `verify_wrfc_format.py`.
6. **Word budgets.** Agent-instruction files stay under `wc -w` ceilings in `scripts/doc_budgets.manifest.json`; a missing budgeted file fails. On red: relocate, condense, raise the ceiling last with a justified manifest diff. Gate: `verify_doc_budgets.py`.
7. **Bilingual pairs.** Every in-scope documentation file ships as `foo.md` beside `foo.zh.md`, equal authority: either side may be authored first; the edited side is the source and the twin follows in the same change as a minimal patch. Header-region language switcher linking the twin (bare filename). Link locale: `.zh.md` pages link `.zh.md` corpus targets, `.md` pages link `.md`. Structural parity: identical heading-depth sequences, byte-identical fenced code blocks (comments included — examples are not translated). Machine tokens and section headings stay English on both sides. No CJK in English-side prose beyond `scripts/doc_languages.manifest.json` exemptions. In Chinese prose: half-width spaces between CJK and Latin; full-width punctuation. Gate: `verify_doc_pairs.py`.

All gates share `scripts/doclib.py`, run in sequence via `scripts/doc_sync.py` (independently runnable, restrictable to given files), and are wired as the prek `doc-check` hook over staged Markdown — full-corpus runs are CI's.

## Slop to hunt

Duplicated homes (the same fact stated in two tiers); narrated history; implementation-status annotations; hand-restated catalogs that a gate or index owns; paragraph walls from hard wrapping; one-sided edits to a language pair.
