#!/usr/bin/env python3
"""PostToolUse hook: format the file that was just edited.

Never blocks (always exits 0). Runs the matching formatter on the edited file
based on its extension and location. Failures are swallowed so a missing tool
or a temporarily-unparseable file never interrupts the agent.

Protocol (Claude Code / Codex — identical): reads JSON on stdin with
`tool_input.file_path`. Exit 0 always; PostToolUse cannot truly block anyway
(the tool already ran).

Formatters:
  .rs                       → rustfmt -w <file>
  frontend .ts/.tsx/...     → prettier --write <rel> (run in frontend/)
  .py                       → ruff format <file>
"""

import json
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BACKEND = PROJECT_ROOT / "backend"
FRONTEND = PROJECT_ROOT / "frontend"

# Frontend extensions handled by prettier.
PRETTIER_EXTS = {".ts", ".tsx", ".js", ".jsx", ".json", ".css", ".md", ".mjs", ".cjs"}


def run(cmd, cwd=None):
    """Run a command, swallowing all errors (formatter must never block)."""
    try:
        subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        sys.exit(0)

    file_path = data.get("tool_input", {}).get("file_path", "")
    if not file_path:
        sys.exit(0)

    abs_path = Path(file_path)
    if not abs_path.is_absolute():
        abs_path = PROJECT_ROOT / file_path

    if not abs_path.exists():
        sys.exit(0)

    suffix = abs_path.suffix

    # Rust: rustfmt on the single file (fast; clippy has no single-file mode).
    if suffix == ".rs":
        run(["rustfmt", "-w", str(abs_path)])
    # Frontend: prettier --write on the single file, run inside frontend/.
    elif suffix in PRETTIER_EXTS and str(abs_path).startswith(str(FRONTEND)):
        try:
            rel = abs_path.relative_to(FRONTEND)
            run(["pnpm", "exec", "prettier", "--write", str(rel)], cwd=FRONTEND)
        except ValueError:
            pass
    # Python: ruff format (devops scripts, hooks themselves).
    elif suffix == ".py":
        run(["ruff", "format", str(abs_path)])

    sys.exit(0)


if __name__ == "__main__":
    main()
