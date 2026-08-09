#!/bin/bash
# 压测编排（load.md §9.1）。
# 1. 启动环境（e2e compose + load override）
# 2. 编译并运行 seed（10 万 users+agents）
# 3. 导出 pairing_code 池
# 4. 启动宿主监控 + RSS 采集（后台）
# 5. 运行 k6（--memory=8g 保护宿主）
# 6. 回归分析
# 7. PG 健康检查
# 8. 清理
set -e

cd "$(dirname "$0")"
E2E_DIR="$(cd ../../e2e && pwd)"
BACKEND_DIR="$(cd ../../backend && pwd)"
LOAD_DIR="$(pwd)"

echo "===== 1. 启动环境 ====="
docker compose -f "$E2E_DIR/docker-compose.e2e.yml" -f "$LOAD_DIR/docker-compose.load.yml" up -d --build
echo "等待 app 健康..."
until curl -sfk https://localhost:8443/api/v1/health; do
  sleep 2
done
echo ""

echo "===== 2. 运行 seed（10 万 users+agents）====="
cd "$BACKEND_DIR"
cargo run --release -p wakewake-seed -- --count 100000 \
  --dsn "postgres://wakewake:test@localhost:5432/wakewake_test"

echo "===== 3. 导出 pairing_code 池 ====="
docker exec e2e-postgres-1 psql -U wakewake -d wakewake_test -t -A \
  -c "SELECT pairing_code FROM agents" > "$LOAD_DIR/pairing_codes.txt"
echo "$(wc -l < "$LOAD_DIR/pairing_codes.txt") codes exported"

echo "===== 4. 启动宿主监控 + RSS 采集（后台）====="
"$LOAD_DIR/monitor-host.sh" &
MONITOR_PID=$!
"$LOAD_DIR/collect-metrics.sh" &
COLLECT_PID=$!

echo "===== 5. 运行压测（k6 优先，Node fallback）====="
mkdir -p "$LOAD_DIR/results"
# 优先 k6（load.md §6.2 规划）；k6 不可用时降级 Node fetch SSE（见 analysis-report.md）
if docker run --rm --network=host --memory=8g --memory-swap=8g \
  -v "$LOAD_DIR":/scripts \
  -e BASE_URL=https://localhost:8443 \
  -e PAIRING_CODES_FILE=/scripts/pairing_codes.txt \
  grafana/k6:2.1.0 run /scripts/sse-connections.js 2>/dev/null; then
  echo "[k6 成功]"
else
  echo "[WARN] k6 不可用（镜像拉取/扩展兼容），降级 Node SSE 压测（analysis-report.md）"
  cd "$LOAD_DIR" && node sse-node.js pairing_codes.txt
fi

echo "===== 6. 停止监控 + 采集 ====="
kill $MONITOR_PID $COLLECT_PID 2>/dev/null || true

echo "===== 7. 回归分析（纯 Python，无 numpy 依赖）====="
python3 "$LOAD_DIR/analyze-regression.py" \
  "$LOAD_DIR/metrics-samples.csv" \
  "$LOAD_DIR/results/report.json" > "$LOAD_DIR/results/regression.md" || echo "[WARN] regression analysis failed"

echo "===== 8. PG 健康检查 ====="
docker exec e2e-postgres-1 psql -U wakewake -d wakewake_test \
  -f "$LOAD_DIR/pg-health-checks.sql" > "$LOAD_DIR/results/pg-health.md" || echo "[WARN] pg health check failed"

echo "===== 9. 清理 ====="
docker compose -f "$E2E_DIR/docker-compose.e2e.yml" down -v

echo ""
echo "压测完成。结果在 $LOAD_DIR/results/"
echo "  - report.json（k6 原始数据）"
echo "  - summary.md（k6 摘要）"
echo "  - metrics-samples.csv（RSS/CPU/PG 采样）"
echo "  - regression.md（线性回归 + 红线断言）"
echo "  - pg-health.md（PG 健康检查）"
