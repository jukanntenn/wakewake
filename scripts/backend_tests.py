#!/usr/bin/env python3
"""Backend test gate: `cargo test --workspace` against a real PG when reachable, else `cargo test --lib`.

Quality gates must not drift between prek and CI (both invoke this script):

  - TEST_DATABASE_URL set (CI service container) -> workspace tests as-is.
  - Local: probe the dev compose PG on localhost:5432 (devops/dev.py stack,
    credentials from devops/.env). Integration tests wipe tables, so they run
    against an isolated `wakewake_test` database (never the dev DB). The DB is
    created via `docker exec ... createdb` on the container publishing 5432.
  - No PG reachable (or creation fails) -> fall back to `cargo test --lib`
    with a loud warning that integration tests only run where a PG exists.

prek runs this with CWD = backend/; the repo root is derived from __file__.
"""

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
BACKEND_DIR = REPO_ROOT / "backend"
PG_HOST = "localhost"
PG_PORT = 5432
TEST_DB = "wakewake_test"


def warn(msg: str) -> None:
    print(f"backend-tests: {msg}", file=sys.stderr)


def read_dev_pg_creds() -> tuple[str, str]:
    """POSTGRES_USER / POSTGRES_PASSWORD from devops/.env (dev compose defaults)."""
    user, password = "wakewake", "wakewake_dev_password"
    env_file = REPO_ROOT / "devops" / ".env"
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            if line.startswith("POSTGRES_USER="):
                user = line.split("=", 1)[1].strip()
            elif line.startswith("POSTGRES_PASSWORD="):
                password = line.split("=", 1)[1].strip()
    return user, password


def probe_tcp(host: str, port: int, timeout: float = 1.0) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def ensure_test_db(user: str) -> bool:
    """Create wakewake_test in the container publishing 5432 (idempotent)."""
    result = subprocess.run(
        ["docker", "ps", "--quiet", "--filter", f"publish={PG_PORT}"],
        capture_output=True,
        text=True,
    )
    container = result.stdout.splitlines()[0].strip() if result.returncode == 0 else ""
    if not container:
        return False
    result = subprocess.run(
        ["docker", "exec", container, "createdb", "-U", user, TEST_DB],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 and f'"{TEST_DB}" already exists' not in result.stderr:
        warn(f"createdb failed: {result.stderr.strip()}")
        return False
    return True


def wait_pg_ready(dsn_port: int, timeout: float = 30.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if probe_tcp(PG_HOST, dsn_port, timeout=2.0):
            return True
        time.sleep(1.0)
    return False


def run_cargo(args: list[str], env: dict[str, str] | None = None) -> int:
    env_full = os.environ.copy()
    if env:
        env_full.update(env)
    return subprocess.run(args, cwd=BACKEND_DIR, env=env_full).returncode


def main() -> int:
    if dsn := os.environ.get("TEST_DATABASE_URL"):
        warn(f"TEST_DATABASE_URL set, running full workspace tests against {dsn.split('@')[-1]}")
        return run_cargo(["cargo", "test", "--workspace"], {"TEST_DATABASE_URL": dsn})

    if not probe_tcp(PG_HOST, PG_PORT):
        warn(
            "no PG reachable on localhost:5432 (dev stack down?) — running unit tests "
            "only; integration tests will run in CI"
        )
        return run_cargo(["cargo", "test", "--lib"])

    user, password = read_dev_pg_creds()
    if not ensure_test_db(user):
        warn("could not ensure wakewake_test database — running unit tests only")
        return run_cargo(["cargo", "test", "--lib"])

    if not wait_pg_ready(PG_PORT):
        warn("PG on 5432 never became ready — running unit tests only")
        return run_cargo(["cargo", "test", "--lib"])

    dsn = f"postgres://{user}:{password}@{PG_HOST}:{PG_PORT}/{TEST_DB}"
    warn(f"running full workspace tests against isolated local PG ({TEST_DB})")
    return run_cargo(["cargo", "test", "--workspace"], {"TEST_DATABASE_URL": dsn})


if __name__ == "__main__":
    sys.exit(main())
