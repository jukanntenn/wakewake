#!/usr/bin/env python3
"""Monthly restore drill: prove the newest offsite dump actually restores.

An untested backup is a hypothesis. The drill pulls the newest dump from the
object store, restores it into a throwaway postgres container, compares row
counts against the live database, and removes every trace — so "backup works"
is a periodically re-verified fact, not an assumption discovered during an
incident.

Slack: the drill compares users (changes are rare and operator-driven) for
equality within the retention window of writes, and wakes for "no more than
a day of writes missing" — the daily dump's worst-case lag.

Exit codes: 0 — drill passed; 1 — drill failed (restored counts off, or the
restore itself errored); 2 — backup.env misconfigured / no dump to restore.
"""

import argparse
import subprocess
import sys
import time

from backup_check import load_env, rclone_binary

DRILL_CONTAINER = "wakewake-backup-drill"
WAKES_SLACK = 2000


def compose_psql(project_dir: str, sql: str) -> str:
    proc = subprocess.run(
        [
            "docker",
            "compose",
            "exec",
            "-T",
            "postgres",
            "psql",
            "-U",
            "wakewake",
            "-d",
            "wakewake",
            "-tAc",
            sql,
        ],
        cwd=project_dir,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"live psql failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def drill_psql(sql: str, db: str = "postgres") -> str:
    # 管理语句（建角色/建库）打 postgres 库；恢复与行数对账打 wakewake 库。
    proc = subprocess.run(
        [
            "docker",
            "exec",
            "-i",
            DRILL_CONTAINER,
            "psql",
            "-U",
            "postgres",
            "-d",
            db,
            "-tAc",
            sql,
        ],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"drill psql failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def count_sql(table: str) -> str:
    return f"SELECT count(*) FROM {table}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project_dir", help="compose project directory")
    args = parser.parse_args()

    env = {**load_env(args.project_dir)}
    for required in (
        "B2_S3_ENDPOINT",
        "B2_S3_REGION",
        "B2_ACCESS_KEY_ID",
        "B2_SECRET_ACCESS_KEY",
    ):
        if not env.get(required):
            print(f"ERROR: {required} missing from backup.env", file=sys.stderr)
            return 2
    bucket = env.get("B2_BUCKET", "wakewake-backups")

    newest = subprocess.run(
        [
            rclone_binary(),
            "lsl",
            f":s3,provider=Other,endpoint={env['B2_S3_ENDPOINT']}:{bucket}/dumps/",
            "--s3-access-key-id",
            env["B2_ACCESS_KEY_ID"],
            "--s3-secret-access-key",
            env["B2_SECRET_ACCESS_KEY"],
            "--s3-region",
            env["B2_S3_REGION"],
            "--s3-no-check-bucket",
        ],
        cwd=args.project_dir,
        capture_output=True,
        text=True,
    )
    if newest.returncode != 0 or not newest.stdout.strip():
        print("ERROR: no dumps listed remotely", file=sys.stderr)
        return 2
    # lsl output: "size date name"; pick the lexically newest name — the
    # stamp prefix (wakewake-YYYYmmddTHHMM) makes lexical order chronological.
    dump_name = sorted(newest.stdout.splitlines())[-1].split()[-1]
    dump_path = f"{bucket}/dumps/{dump_name}"

    live_users = int(compose_psql(args.project_dir, count_sql("users")))
    live_wakes = int(compose_psql(args.project_dir, count_sql("wakes")))

    failures: list[str] = []
    try:
        subprocess.run(["docker", "rm", "-f", DRILL_CONTAINER], capture_output=True)
        run = subprocess.run(
            [
                "docker",
                "run",
                "-d",
                "--rm",
                "--name",
                DRILL_CONTAINER,
                "-e",
                "POSTGRES_PASSWORD=drill",
                "postgres:17-alpine",
            ],
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            raise RuntimeError(f"drill container failed to start: {run.stderr.strip()}")
        for _ in range(30):
            ready = subprocess.run(
                ["docker", "exec", DRILL_CONTAINER, "pg_isready", "-U", "postgres"],
                capture_output=True,
            )
            if ready.returncode == 0:
                break
            time.sleep(2)
        else:
            raise RuntimeError("drill postgres never became ready")

        # The dump was taken with --no-owner and against db name wakewake;
        # create the role + database so the restore target matches.
        drill_psql("CREATE ROLE wakewake LOGIN PASSWORD 'drill'")
        drill_psql("CREATE DATABASE wakewake OWNER wakewake")
        # Stream object store -> zstd -> psql in one pipe.
        cat = subprocess.Popen(
            [
                rclone_binary(),
                "cat",
                f":s3,provider=Other,endpoint={env['B2_S3_ENDPOINT']}:{dump_path}",
                "--s3-access-key-id",
                env["B2_ACCESS_KEY_ID"],
                "--s3-secret-access-key",
                env["B2_SECRET_ACCESS_KEY"],
                "--s3-region",
                env["B2_S3_REGION"],
                "--s3-no-check-bucket",
            ],
            stdout=subprocess.PIPE,
        )
        zstd = subprocess.Popen(
            ["zstd", "-dc"], stdin=cat.stdout, stdout=subprocess.PIPE
        )
        psql = subprocess.run(
            [
                "docker",
                "exec",
                "-i",
                DRILL_CONTAINER,
                "psql",
                "-U",
                "postgres",
                "-d",
                "wakewake",
                "-q",
            ],
            stdin=zstd.stdout,
            capture_output=True,
            text=True,
        )
        assert cat.stdout and zstd.stdout
        cat.stdout.close()
        zstd.stdout.close()
        codes = [proc.wait() for proc in (cat, zstd)]
        if psql.returncode != 0 or any(code != 0 for code in codes):
            raise RuntimeError(
                f"restore pipeline failed {codes} psql={psql.returncode}: {psql.stderr[:300]}"
            )

        drill_users = int(drill_psql(count_sql("users"), "wakewake"))
        drill_wakes = int(drill_psql(count_sql("wakes"), "wakewake"))
        print(
            f"users live={live_users} drill={drill_users}; "
            f"wakes live={live_wakes} drill={drill_wakes}"
        )
        if drill_users < live_users:
            failures.append(f"drill users {drill_users} < live {live_users}")
        if drill_wakes + WAKES_SLACK < live_wakes:
            failures.append(f"drill wakes {drill_wakes} far behind live {live_wakes}")
    except RuntimeError as exc:
        failures.append(str(exc))
    finally:
        subprocess.run(["docker", "rm", "-f", DRILL_CONTAINER], capture_output=True)

    if failures:
        print(f"ERROR: {'; '.join(failures)}", file=sys.stderr)
        return 1
    print(f"drill passed against {dump_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
