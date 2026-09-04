#!/usr/bin/env python3
"""Wakewake development environment manager.

Infrastructure (postgres + mailpit) runs in Docker; backend and frontend run as
host processes for hot reload:

    python3 devops/dev.py start    # infra + backend (cargo watch) + frontend (pnpm dev)
    python3 devops/dev.py init     # infra only + generate backend/config.local.toml
    python3 devops/dev.py stop     # stop everything

backend/config.local.toml is generated on first start (dev defaults, gitignored);
edit freely afterwards. Full schema: backend/config.example.toml.

Ports: backend 8080, frontend ${FRONTEND_PORT:-3034} (matches frontend/package.json),
postgres 5432, mailpit UI 8025 / SMTP 1025.
"""

import argparse
import logging
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent
BACKEND_DIR = PROJECT_ROOT / "backend"
COMPOSE_FILE = SCRIPT_DIR / "dev-compose.yml"

BACKEND_PORT = 8080
FRONTEND_PORT = os.environ.get("FRONTEND_PORT", "3034")

# (name, pid-file, log-file) —— 宿主进程登记表，start 写 / stop 读。
HOST_PROCS = [
    ("backend", SCRIPT_DIR / "backend.pid", SCRIPT_DIR / "backend.log"),
    ("frontend", SCRIPT_DIR / "frontend.pid", SCRIPT_DIR / "frontend.log"),
]

# dev 默认配置（首次 start 生成 backend/config.local.toml；之后不再覆盖）。
DEV_CONFIG = """\
# 开发环境配置（devops/dev.py 首次启动自动生成，可自行修改；*.local.toml 不入 git）。
# 完整 schema 见 backend/config.example.toml（WAKEWAKE_* 环境变量可覆盖任意项）。

[app]
public_url = "http://localhost:{frontend_port}"

[database]
dsn = "postgres://wakewake:wakewake@localhost:5432/wakewake_dev"

[jwt]
signing_key = "dev-only-signing-key-0123456789ab"
refresh_signing_key = "dev-only-refresh-key-0123456789abcd"

[password_reset]
secret = "dev-only-reset-secret-0123456789ab"

# mailpit（dev-compose.yml）：SMTP 1025，Web UI http://localhost:8025
[mailer]
enabled = true
smtp_host = "localhost"
smtp_port = 1025
from_address = "noreply@wakewake.local"
from_name = "WakeWake Dev"
"""

logger = logging.getLogger("dev")


def setup_logging():
    handler_out = logging.StreamHandler(sys.stdout)
    handler_out.setLevel(logging.INFO)
    handler_out.addFilter(lambda record: record.levelno <= logging.INFO)

    handler_err = logging.StreamHandler(sys.stderr)
    handler_err.setLevel(logging.WARNING)

    logging.basicConfig(level=logging.INFO, handlers=[handler_out, handler_err])


def parse_args():
    parser = argparse.ArgumentParser(
        description="Wakewake development environment manager"
    )
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("start", help="infra + backend + frontend (default)")
    sub.add_parser("init", help="infra only + generate backend/config.local.toml")
    sub.add_parser("stop", help="stop everything")
    args = parser.parse_args()
    if not args.command:
        args.command = "start"
    return args


def run(name, *args, **kwargs):
    defaults = {"stdout": subprocess.PIPE, "stderr": subprocess.PIPE, "text": True}
    defaults.update(kwargs)
    return subprocess.run([name, *args], **defaults)


def compose(*args):
    return run("docker", "compose", "-f", str(COMPOSE_FILE), *args)


def wait_for_http(url, name, timeout):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            if run("curl", "-sf", url).returncode == 0:
                logger.info("[ok] %s is ready", name)
                return True
        except Exception:
            pass
        time.sleep(2)
    logger.warning("[warn] %s not ready after %ds, continuing...", name, timeout)
    return False


def wait_for_pg(timeout=60):
    deadline = time.time() + timeout
    while time.time() < deadline:
        result = compose("exec", "-T", "postgres", "pg_isready", "-U", "wakewake")
        if result.returncode == 0:
            logger.info("[ok] PostgreSQL is ready")
            return True
        time.sleep(2)
    logger.warning("[warn] PostgreSQL not ready after %ds, continuing...", timeout)
    return False


def ensure_dev_config():
    config = BACKEND_DIR / "config.local.toml"
    if config.exists():
        return
    config.write_text(DEV_CONFIG.format(frontend_port=FRONTEND_PORT), encoding="utf-8")
    logger.info("[ok] generated %s (dev defaults)", config.relative_to(PROJECT_ROOT))


def has_cargo_watch():
    return run("cargo", "watch", "--version").returncode == 0


def spawn(name, cmd, cwd, pid_file, log_file):
    with open(log_file, "w") as log:
        proc = subprocess.Popen(
            cmd,
            cwd=str(cwd),
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
    pid_file.write_text(str(proc.pid))
    logger.info("[ok] %s started (pid %s, log %s)", name, proc.pid, log_file.name)


def start_infra():
    logger.info("Starting infra (postgres + mailpit)...")
    compose("up", "-d")
    wait_for_pg()
    ensure_dev_config()


def start():
    start_infra()

    if has_cargo_watch():
        backend_cmd = [
            "cargo",
            "watch",
            "-x",
            "run -- --config config.local.toml serve",
        ]
    else:
        logger.info(
            "cargo-watch not installed; backend won't auto-reload "
            "(cargo install cargo-watch)"
        )
        backend_cmd = ["cargo", "run", "--", "--config", "config.local.toml", "serve"]
    spawn(
        "backend",
        backend_cmd,
        BACKEND_DIR,
        SCRIPT_DIR / "backend.pid",
        SCRIPT_DIR / "backend.log",
    )
    wait_for_http(
        f"http://localhost:{BACKEND_PORT}/api/v1/health", "Backend", timeout=300
    )

    frontend_dir = PROJECT_ROOT / "frontend"
    if not (frontend_dir / "node_modules").exists():
        logger.info("Installing frontend dependencies...")
        run("pnpm", "install", cwd=str(frontend_dir))
    spawn(
        "frontend",
        ["pnpm", "dev"],
        frontend_dir,
        SCRIPT_DIR / "frontend.pid",
        SCRIPT_DIR / "frontend.log",
    )
    wait_for_http(f"http://localhost:{FRONTEND_PORT}", "Frontend", timeout=60)

    logger.info("Services:")
    logger.info("  Frontend:     http://localhost:%s", FRONTEND_PORT)
    logger.info("  Backend API:  http://localhost:%d/api/v1", BACKEND_PORT)
    logger.info("  Mailpit UI:   http://localhost:8025")
    logger.info("  PostgreSQL:   localhost:5432 (wakewake/wakewake/wakewake_dev)")
    logger.info("Stop: python devops/dev.py stop")


def kill_pid_group(pid_file):
    if not pid_file.exists():
        return
    pid = int(pid_file.read_text().strip())
    try:
        os.killpg(pid, signal.SIGTERM)
        time.sleep(1)
        os.killpg(pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    pid_file.unlink()


def kill_port(port):
    try:
        result = run("lsof", "-ti", f":{port}")
        for pid_str in result.stdout.strip().splitlines():
            subprocess.run(["kill", "-9", pid_str], capture_output=True)
    except Exception:
        pass


def stop():
    for name, pid_file, _log in HOST_PROCS:
        logger.info("Stopping %s...", name)
        kill_pid_group(pid_file)
    kill_port(BACKEND_PORT)
    kill_port(int(FRONTEND_PORT))
    logger.info("Stopping infra...")
    compose("down")
    logger.info("All stopped")


def main():
    setup_logging()
    args = parse_args()
    if args.command == "start":
        start()
    elif args.command == "init":
        start_infra()
    elif args.command == "stop":
        stop()


if __name__ == "__main__":
    main()
