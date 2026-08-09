#!/bin/bash
# 宿主内存监控兜底（load.md §10.2）。
# 可用内存 <1.5GB 时提前终止 k6（保护宿主，避免 OOM kill 系统进程）。
THRESHOLD_MB=1500

cd "$(dirname "$0")"
while true; do
  avail=$(free -m | awk '/Mem:/{print $7}')
  if [ -z "$avail" ]; then
    sleep 5
    continue
  fi
  if [ "$avail" -lt "$THRESHOLD_MB" ]; then
    echo "[ALERT] Available memory ${avail}MB < ${THRESHOLD_MB}MB, stopping k6" >&2
    # 找宿主 k6 进程（docker run 的 k6）或容器内 k6
    pkill -SIGTERM k6 2>/dev/null || true
    docker stop wakewake-load-k6 2>/dev/null || true
    break
  fi
  sleep 5
done
