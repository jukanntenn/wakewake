#!/usr/bin/env python3
"""Sync .codex/hooks from .claude/hooks (.claude is the source of truth).

The hook scripts are agent-agnostic: both Claude Code and Codex read
`tool_input.file_path` from stdin JSON, honor `stop_hook_active`, and block via
JSON {decision: block} on exit 0. So one set of scripts serves both.

The difference is path resolution in the config file:
  - Claude Code resolves `$CLAUDE_PROJECT_DIR` at runtime.
  - Codex has no native project-dir variable, so we bake the absolute path
    into the generated `.codex/hooks.json`.

Run after editing .claude/hooks/* or after cloning to a new machine.

Usage: python3 scripts/sync_hooks.py
"""

import json
from pathlib import Path
import shutil
import sys


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    claude_hooks = root / ".claude" / "hooks"
    codex_hooks = root / ".codex" / "hooks"
    codex_settings = root / ".codex" / "hooks.json"

    if not claude_hooks.exists():
        print(f"error: {claude_hooks} does not exist", file=sys.stderr)
        return 1

    # Copy the agent-agnostic scripts verbatim.
    codex_hooks.mkdir(parents=True, exist_ok=True)
    for name in ("format.py", "lint_stop.py"):
        src = claude_hooks / name
        if src.exists():
            shutil.copy2(src, codex_hooks / name)

    # Codex hooks.json: same schema as .claude/settings.json hooks block, but
    # with absolute paths (Codex has no $CLAUDE_PROJECT_DIR native variable).
    fmt_cmd = f"python3 {codex_hooks / 'format.py'}"
    lint_cmd = f"python3 {codex_hooks / 'lint_stop.py'}"

    config = {
        "hooks": {
            "PostToolUse": [
                {
                    "matcher": "Edit|Write|MultiEdit",
                    "hooks": [{"type": "command", "command": fmt_cmd}],
                }
            ],
            "Stop": [
                {"hooks": [{"type": "command", "command": lint_cmd}]}
            ],
        }
    }

    (root / ".codex").mkdir(parents=True, exist_ok=True)
    codex_settings.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")

    print("Synced .codex/hooks + .codex/hooks.json <- .claude/hooks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
