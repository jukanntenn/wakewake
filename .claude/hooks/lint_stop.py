#!/usr/bin/env python3
"""Stop hook: run lint before the agent finishes; block on errors.

Runs cargo fmt --check + cargo clippy (backend) and pnpm lint + prettier
--check (frontend). On any error, prints JSON {decision: block, reason: ...}
and exits 0 (exit 0 + JSON is the documented way to block a Stop hook; exit 2
ignores JSON and uses stderr instead).

Protocol (Claude Code / Codex — identical): reads JSON on stdin. Checks
`stop_hook_active`; if true, exits 0 to avoid infinite loops (the host forces
stop after 8 consecutive blocks anyway).

NOTE on clippy: the project runs clippy at pedantic level. Existing code has
a ~70-warning backlog, so `-D warnings` is NOT yet passed here — the command
is `cargo clippy --all-targets --all-features` without -D. Once the backlog is
cleared, add `-- -D warnings` to enforce hard failure. New code must still be
clippy-clean.
"""

import json
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BACKEND = PROJECT_ROOT / "backend"
FRONTEND = PROJECT_ROOT / "frontend"


def run_check(cmd, cwd):
    """Run a command; return None on success, the combined output on failure.

    Returns None (don't block) if the tool is missing or times out.
    """
    try:
        r = subprocess.run(
            cmd,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=300,
        )
        if r.returncode == 0:
            return None
        return (r.stdout + r.stderr).strip() or "(no output)"
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None  # tool missing/unavailable: don't block


def block(reason):
    print(json.dumps({"decision": "block", "reason": reason}))
    sys.exit(0)


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        sys.exit(0)

    # Already looping from a previous block; let the agent stop.
    if data.get("stop_hook_active"):
        sys.exit(0)

    errors = []

    # Backend: fmt check + clippy (no -D warnings until pedantic backlog cleared).
    if (BACKEND / "Cargo.toml").exists():
        err = run_check(["cargo", "fmt", "--check"], BACKEND)
        if err:
            errors.append("cargo fmt --check (backend):\n" + err)
        err = run_check(
            ["cargo", "clippy", "--all-targets", "--all-features"], BACKEND
        )
        if err:
            errors.append(
                "cargo clippy (backend) — note: -D warnings not yet enforced "
                "due to pedantic backlog:\n" + err
            )

    # Frontend: lint + format check.
    if (FRONTEND / "package.json").exists():
        err = run_check(["pnpm", "lint"], FRONTEND)
        if err:
            errors.append("eslint (frontend):\n" + err)
        err = run_check(["pnpm", "exec", "prettier", "--check", "."], FRONTEND)
        if err:
            errors.append("prettier --check (frontend):\n" + err)

    if errors:
        block(
            "Lint/format errors must be fixed before finishing:\n\n"
            + "\n\n".join(errors)
        )
    sys.exit(0)


if __name__ == "__main__":
    main()
