#!/usr/bin/env python3
"""Daily logical dump: pg_dump streamed through zstd into the object store.

The dump is the restore path of last resort and the format-diversity hedge:
a plain SQL dump survives base-image or page-level corruption that would
break any block-level scheme, restores onto any Postgres 17 instance, and is
human-readable in an emergency. rclone is rate-limited so a thin uplink
keeps headroom for origin traffic. Credentials come from backup.env (0600,
vaulted) — an add-only application key shared by nothing else.

Exit codes: 0 — dump stored; 1 — a pipeline stage failed; 2 — backup.env
misconfigured.
"""

import argparse
import datetime
import subprocess
import sys

from backup_check import load_env, rclone_binary


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
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M")
    remote = (
        f":s3,provider=Other,endpoint={env['B2_S3_ENDPOINT']}:"
        f"{bucket}/dumps/wakewake-{stamp}.sql.zst"
    )

    # --no-owner: the dump must restore on any instance regardless of which
    # roles exist; ownership is irrelevant for a single-user application DB.
    dump = subprocess.Popen(
        [
            "docker",
            "compose",
            "exec",
            "-T",
            "postgres",
            "pg_dump",
            "-U",
            "wakewake",
            "--no-owner",
            "wakewake",
        ],
        cwd=args.project_dir,
        stdout=subprocess.PIPE,
    )
    zstd = subprocess.Popen(
        ["zstd", "-19", "-T2"],
        stdin=dump.stdout,
        stdout=subprocess.PIPE,
    )
    rclone = subprocess.Popen(
        [
            rclone_binary(),
            "rcat",
            remote,
            "--s3-access-key-id",
            env["B2_ACCESS_KEY_ID"],
            "--s3-secret-access-key",
            env["B2_SECRET_ACCESS_KEY"],
            "--s3-region",
            env["B2_S3_REGION"],
            "--s3-no-check-bucket",
            "--bwlimit",
            "2M",
        ],
        stdin=zstd.stdout,
    )
    assert dump.stdout and zstd.stdout
    dump.stdout.close()
    zstd.stdout.close()
    codes = [proc.wait() for proc in (dump, zstd, rclone)]
    if any(code != 0 for code in codes):
        print(f"ERROR: dump pipeline stages exited {codes}", file=sys.stderr)
        return 1
    print(f"stored {remote}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
