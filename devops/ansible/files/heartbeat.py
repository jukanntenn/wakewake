#!/usr/bin/env python3
# wakewake availability heartbeat: probe the loopback-published health endpoint
# and the sibling postgres container, then push the verdict to the uptime-kuma
# push monitor.
#
# kuma flags the monitor down in two independent ways: this script pushes
# status=down (app-level failure — covers the edge blind spot, since the push
# path bypasses the public ingress) or the pushes stop entirely (host death,
# this script crashed). The push URL is secret: anyone holding it can forge
# "up" beats and mask an outage. It therefore arrives via the KUMA_HEARTBEAT_URL
# environment variable (set by the systemd user unit from the ansible vault),
# never baked into this file; the non-secret knobs arrive as command-line
# arguments.
#
# Standard library only, per house rule for script-type programs. The loop
# never exits on purpose; systemd's Restart=always is the recovery path if it
# dies anyway.
#
# Exit codes: 2 — KUMA_HEARTBEAT_URL unset (misconfiguration); the loop itself
# runs forever.

import argparse
import json
import os
import ssl
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


def probe_health(health_url: str, insecure: bool) -> bool:
    # Body must say "ok": a gateway misroute can answer 200 with an error
    # page, and a false "up" here would mask a real outage.
    context = ssl._create_unverified_context() if insecure else None
    try:
        with urllib.request.urlopen(health_url, timeout=5, context=context) as resp:
            return json.load(resp).get("status") == "ok"
    except (urllib.error.URLError, OSError, ValueError):
        return False


def probe_postgres(project_dir: str) -> bool:
    # /api/v1/health is deliberately liveness-only (no DB round trip — the
    # docker healthcheck must not flap on DB load), so the heartbeat adds the
    # DB probe itself: the app without its database cannot serve logins or
    # commands, and pg_isready over the compose exec path exercises the exact
    # socket the app uses.
    try:
        proc = subprocess.run(
            [
                "docker",
                "compose",
                "exec",
                "-T",
                "postgres",
                "pg_isready",
                "-h",
                "/var/run/postgresql",
                "-U",
                "wakewake",
            ],
            cwd=project_dir,
            capture_output=True,
            timeout=15,
        )
    except (subprocess.SubprocessError, OSError):
        return False
    return proc.returncode == 0


def push(push_url: str, status: str, msg: str) -> None:
    query = urllib.parse.urlencode({"status": status, "msg": msg})
    try:
        # A failed push is not fatal: kuma marks the monitor down only after
        # its own retry window without a beat, so one transient blip on the
        # kuma side does not immediately page.
        urllib.request.urlopen(f"{push_url}?{query}", timeout=10).close()
    except (urllib.error.URLError, OSError, ValueError):
        pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="wakewake availability heartbeat")
    parser.add_argument("--health-url", required=True, help="health endpoint to probe")
    parser.add_argument(
        "--project-dir", required=True, help="compose project directory"
    )
    parser.add_argument(
        "--interval", type=int, default=60, help="seconds between beats"
    )
    parser.add_argument(
        "--insecure",
        action="store_true",
        help="skip TLS verification (self-signed environments)",
    )
    args = parser.parse_args(argv)

    push_url = os.environ.get("KUMA_HEARTBEAT_URL", "")
    if not push_url:
        print("KUMA_HEARTBEAT_URL is not set", file=sys.stderr)
        return 2

    while True:
        if probe_health(args.health_url, args.insecure):
            if probe_postgres(args.project_dir):
                push(push_url, "up", "health + postgres ready")
            else:
                push(push_url, "down", "postgres probe failed")
        else:
            push(push_url, "down", "local health probe failed")
        time.sleep(args.interval)


if __name__ == "__main__":
    raise SystemExit(main())
