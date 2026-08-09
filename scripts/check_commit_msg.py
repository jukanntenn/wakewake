#!/usr/bin/env python3
"""commit-msg hook: validate Conventional Commits format.

prek passes the path to Git's commit-message file as $1 (sys.argv[1]).
The script reads the file and checks the first non-comment line against the
Conventional Commits pattern. Exit non-zero to reject the commit.

Allowed types: feat|fix|chore|docs|refactor|test|build|style|ci|perf|revert
Format:        <type>(<scope>)?!?: <summary>
"""

import re
import sys
from pathlib import Path

PATTERN = re.compile(
    r"^(feat|fix|chore|docs|refactor|test|build|style|ci|perf|revert)"
    r"(\([^)]+\))?!?: .+"
)


def main() -> int:
    if len(sys.argv) < 2:
        print("error: commit-msg hook expects the message file path as $1", file=sys.stderr)
        return 1

    msg_file = Path(sys.argv[1])
    try:
        content = msg_file.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        print(f"error: cannot read commit message file {msg_file}: {e}", file=sys.stderr)
        return 1

    # First non-comment, non-blank line is the subject.
    subject = ""
    for line in content.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        subject = stripped
        break

    if not PATTERN.match(subject):
        print("ERROR: commit message must follow Conventional Commits.", file=sys.stderr)
        print("  Expected: <type>(<scope>)?!?: <summary>", file=sys.stderr)
        print(
            "  Types: feat|fix|chore|docs|refactor|test|build|style|ci|perf|revert",
            file=sys.stderr,
        )
        print(f"  Got: {subject}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
