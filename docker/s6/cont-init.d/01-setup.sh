#!/bin/sh
# s6 cont-init.d：容器初始化（建目录、设时区）。
# 复刻参考项目 grill-me-sleek 的 01-setup.sh（e2e.md §2.2）。
set -e

# 建运行时目录
mkdir -p /app/data
mkdir -p /app/frontend/out

# 设时区（若有 TZ env）
if [ -n "$TZ" ]; then
  ln -snf /usr/share/zoneinfo/"$TZ" /etc/localtime 2>/dev/null || true
  echo "$TZ" > /etc/timezone 2>/dev/null || true
fi

echo "[s6-init] wakewake app container setup complete"
