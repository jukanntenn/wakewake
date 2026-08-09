#!/bin/bash
# RSS/CPU/PG 外部采集（load.md §7.3）。
# 每 5s 采一次，CSV 输出。pgrep 定位 server PID（非 PID 1——s6-overlay 容器 PID 1 是 s6-svscan）。
CONTAINER=e2e-app-1
PG_CONTAINER=e2e-postgres-1
OUT=metrics-samples.csv

cd "$(dirname "$0")"
echo "timestamp,app_rss_kb,app_cpu_percent,pg_rss_kb,pg_active_conns,host_avail_mb" > "$OUT"

while true; do
  ts=$(date +%s)
  # server PID（busybox pgrep 不可靠，遍历 /proc 找 wakewake-server）
  SERVER_PID=$(docker exec "$CONTAINER" sh -c 'for p in $(ls /proc/|grep ^[0-9]); do c=$(cat /proc/$p/cmdline 2>/dev/null|tr "\0" " "|head -c 40); case "$c" in *wakewake*) echo $p; break;; esac; done' 2>/dev/null)
  if [ -n "$SERVER_PID" ]; then
    app_rss=$(docker exec "$CONTAINER" cat "/proc/$SERVER_PID/status" 2>/dev/null | awk '/VmRSS/{print $2}')
  else
    app_rss=""
  fi
  app_cpu=$(docker stats --no-stream --format "{{.CPUPerc}}" "$CONTAINER" 2>/dev/null | tr -d ' %')
  pg_rss=$(docker exec "$PG_CONTAINER" cat /proc/1/status 2>/dev/null | awk '/VmRSS/{print $2}')
  pg_conns=$(docker exec "$PG_CONTAINER" psql -U wakewake -d wakewake_test -t -A \
    -c "SELECT count(*) FROM pg_stat_activity WHERE state='active'" 2>/dev/null)
  host_avail=$(free -m | awk '/Mem:/{print $7}')
  echo "$ts,$app_rss,$app_cpu,$pg_rss,$pg_conns,$host_avail" >> "$OUT"
  sleep 5
done
