#!/usr/bin/env python3
"""Sync / check CLAUDE.md against AGENTS.md (AGENTS.md is the source of truth).

Two modes:
  default      — copy AGENTS.md → CLAUDE.md (run after editing AGENTS.md)
  --check      — verify they are identical; exit 1 if not (used by the prek
                 agents-sync hook so a stale CLAUDE.md blocks the commit)

Usage:
  python3 scripts/sync_agents.py          # sync
  python3 scripts/sync_agents.py --check  # check only
"""

from pathlib import Path
import shutil
import sys


def main() -> int:
    check_only = "--check" in sys.argv[1:]

    root = Path(__file__).resolve().parents[1]
    source = root / "AGENTS.md"
    target = root / "CLAUDE.md"

    if not source.exists():
        print(f"error: {source} does not exist", file=sys.stderr)
        return 1

    if check_only:
        if not target.exists():
            print(
                "AGENTS.md != CLAUDE.md — CLAUDE.md is missing. "
                "Run: python3 scripts/sync_agents.py",
                file=sys.stderr,
            )
            return 1
        if source.read_bytes() != target.read_bytes():
            print(
                "AGENTS.md != CLAUDE.md — out of sync. "
                "Run: python3 scripts/sync_agents.py",
                file=sys.stderr,
            )
            return 1
        return 0

    shutil.copy2(source, target)
    print("Synced CLAUDE.md <- AGENTS.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
