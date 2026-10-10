#!/usr/bin/env python3
"""Daily backup observability.

Two probes, one verdict pushed to the uptime-kuma monitor so failure shows up
as an explicit down push (or, if even this script dies, as silence):

  1. freshness — the newest offsite dump older than 26 h means silent backup
     death (missed timer, full disk, expired object-store key).
  2. local database liveness — the dump source itself must be reachable, or
     every future dump fails too.

KUMA_BACKUP_URL is optional: sourced from backup.env (0600, vaulted) when the
monitor exists; without it the script still fails loudly via its exit code
and the journal.

Exit codes: 0 — healthy; 1 — a probe failed; 2 — backup.env misconfigured.
"""

import argparse
import datetime
import pathlib
import subprocess
import sys
import urllib.parse
import urllib.request

FRESHNESS_SECONDS = 26 * 3600


def load_env(project_dir: str) -> dict[str, str]:
    """Overlay backup.env (quoted KEY=VALUE lines) on the environ."""
    env = dict()
    path = pathlib.Path(project_dir) / "backup.env"
    if path.exists():
        for line in path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                key, value = line.split("=", 1)
                value = value.strip()
                # strip one pair of matching quotes the template adds so shell
                # sourcing stays safe for values containing & characters
                if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
                    value = value[1:-1]
                env[key.strip()] = value
    return env


def compose_exec(argv: list[str], project_dir: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["docker", "compose", "exec", "-T", "postgres"] + argv,
        cwd=project_dir,
        capture_output=True,
        text=True,
    )


def rclone_binary() -> str:
    # The static binary is installed user-local (~/.local/bin) when the host
    # package is absent; user units do not inherit the login PATH.
    import shutil

    return shutil.which("rclone") or str(pathlib.Path.home() / ".local/bin/rclone")


def newest_dump_age_seconds(project_dir: str) -> float | None:
    env = load_env(project_dir)
    for required in ("B2_S3_ENDPOINT", "B2_ACCESS_KEY_ID", "B2_SECRET_ACCESS_KEY"):
        if not env.get(required):
            print(f"ERROR: {required} missing from backup.env", file=sys.stderr)
            raise SystemExit(2)
    remote = (
        f":s3,provider=Other,endpoint={env['B2_S3_ENDPOINT']}:"
        f"{env.get('B2_BUCKET', 'wakewake-backups')}/dumps/"
    )
    proc = subprocess.run(
        [
            rclone_binary(),
            "lsl",
            remote,
            "--s3-access-key-id",
            env["B2_ACCESS_KEY_ID"],
            "--s3-secret-access-key",
            env["B2_SECRET_ACCESS_KEY"],
            "--s3-region",
            env.get("B2_S3_REGION", "us-east-005"),
            "--s3-no-check-bucket",
        ],
        cwd=project_dir,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return None
    # rclone lsl lines: "size modification-date name" (date is server-local
    # for B2, UTC in practice for S3-compatible listings; both are far from
    # the 26 h threshold).
    stamps = []
    for line in proc.stdout.splitlines():
        parts = line.split()
        if len(parts) >= 3:
            try:
                stamps.append(
                    datetime.datetime.strptime(parts[1], "%Y-%m-%d").replace(
                        tzinfo=datetime.timezone.utc
                    )
                )
            except ValueError:
                continue
    if not stamps:
        return None
    newest = max(stamps)
    return (datetime.datetime.now(datetime.timezone.utc) - newest).total_seconds()


def push_verdict(url: str, ok: bool, msg: str) -> None:
    # Best-effort: a failed push is caught by kuma's silence detection — the
    # same dual channel the availability heartbeat uses. The vaulted URL is
    # the bare push endpoint (no query suffix): appending with "?" keeps a
    # single status/msg pair — a second "?status=..." suffix from the kuma
    # copy-paste form would produce duplicate params and kuma reads the
    # status array as "not up", marking the monitor down on every push.
    query = urllib.parse.urlencode({"status": "up" if ok else "down", "msg": msg})
    try:
        urllib.request.urlopen(f"{url}?{query}", timeout=10)
    except OSError:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project_dir", help="compose project directory")
    args = parser.parse_args()

    failures: list[str] = []

    age = newest_dump_age_seconds(args.project_dir)
    if age is None or age > FRESHNESS_SECONDS:
        failures.append(f"no dump newer than 26h (age={age}s)")

    probe = compose_exec(
        ["pg_isready", "-h", "/var/run/postgresql", "-U", "wakewake"],
        args.project_dir,
    )
    if probe.returncode != 0:
        failures.append("local postgres unreachable — next dump will fail")

    kuma_url = load_env(args.project_dir).get("KUMA_BACKUP_URL")
    if kuma_url:
        push_verdict(kuma_url, not failures, "; ".join(failures) or "ok")

    if failures:
        print(f"ERROR: {'; '.join(failures)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
