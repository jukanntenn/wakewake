#!/bin/bash
# prod-sim 压测编排：生产镜像 + 2c/2g 资源限制 + 3Mbps 出口整形 + 限流开启。
#
# 与 run.sh（裸机、不限资源、关限流）互补；场景矩阵 R1–R7 见 specs/testing/load.md。
#
# 用法：
#   SCENARIO=r3-reconnect-storm ./run-prod-sim.sh        # 重连风暴（会自动重启 app）
#   SCENARIO=r4-dashboard-mix ./run-prod-sim.sh          # 仪表盘混合负载（限流开启）
#   SCENARIO=r1-sse-knee SEED_COUNT=50000 ./run-prod-sim.sh
#
# 环境变量：
#   SCENARIO              k6 脚本名（tests/load/<SCENARIO>.js，默认 sse-connections）
#   SEED_COUNT            seed 用户/agent 数（默认 12000）
#   WAN_RATE              出口整形速率（默认 3mbit；0 = 不整形）
#   WAN_DELAY             附加单向延迟（默认 0；如 40ms 模拟公网/CF 路径）
#   LOAD_DISABLE_RATE_LIMIT 1 = 关闭限流（仅硬件天花板探测时用，结果须标注）
#   K6_IMAGE              k6 镜像（默认 wakewake-k6-sse:1.2.1，本地构建含 xk6-sse）
#   R3_RESTART_AFTER      R3 场景 k6 启动多少秒后重启 app（默认 180）
#   KEEP                  1 = 结束不销毁环境（便于追加场景）
set -euo pipefail

LOAD_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$LOAD_DIR/../.." && pwd)
SCENARIO=${SCENARIO:-sse-connections}
SEED_COUNT=${SEED_COUNT:-12000}
WAN_RATE=${WAN_RATE:-3mbit}
WAN_DELAY=${WAN_DELAY:-0ms}
K6_IMAGE=${K6_IMAGE:-wakewake-k6-sse:1.2.1}
R3_RESTART_AFTER=${R3_RESTART_AFTER:-180}
APP_CONTAINER=e2e-app-1
PG_CONTAINER=e2e-postgres-1
NETWORK=wakewake-load-net
# e2e compose 凭据（独立测试密钥，绝不含生产 secret）
PG_DSN="postgres://wakewake:test@localhost:15432/wakewake_test"

COMPOSE=(docker compose -f "$REPO_ROOT/e2e/docker-compose.e2e.yml" -f "$LOAD_DIR/docker-compose.prod-sim.yml")

mkdir -p "$LOAD_DIR/results"

# 在 app netns 内执行 tc（wan-shaper 容器，免改生产镜像、免 root）。
# --cap-add NET_ADMIN 必需：共享 netns 不继承网络管理能力，能力随容器进程走。
tc_in_app() {
  docker run --rm --network "container:$APP_CONTAINER" --cap-add NET_ADMIN \
    wakewake-wan-shaper "$@"
}

apply_shaping() {
  if [ "$WAN_RATE" = "0" ] && [ "$WAN_DELAY" = "0ms" ]; then
    echo "[shaping] 已禁用（WAN_RATE=0 且 WAN_DELAY=0ms）"
    return
  fi
  if [ "$WAN_DELAY" != "0ms" ] && [ "$WAN_RATE" != "0" ]; then
    # netem 根 + tbf 子（netem 隐含类 1:1）
    tc_in_app qdisc replace dev eth0 root handle 1: netem delay $WAN_DELAY # shellcheck disable=SC2086
    # WAN_DELAY="30ms 5ms" 需拆两 argv(delay+jitter)
    tc_in_app qdisc replace dev eth0 parent 1:1 handle 10: tbf rate "$WAN_RATE" burst 32kb latency 400ms
  elif [ "$WAN_DELAY" != "0ms" ]; then
    tc_in_app qdisc replace dev eth0 root netem delay $WAN_DELAY # shellcheck disable=SC2086
  else
    tc_in_app qdisc replace dev eth0 root tbf rate "$WAN_RATE" burst 32kb latency 400ms
  fi
  echo "[shaping] egress = rate $WAN_RATE, delay $WAN_DELAY"
}

echo "===== 0. 构建辅助镜像（k6+xk6-sse / wan-shaper，已存在则跳过）====="
docker image inspect "$K6_IMAGE" >/dev/null 2>&1 || \
  docker build -t "$K6_IMAGE" -f "$LOAD_DIR/k6-sse.Dockerfile" "$LOAD_DIR"
docker image inspect wakewake-wan-shaper >/dev/null 2>&1 || \
  docker build -t wakewake-wan-shaper "$LOAD_DIR/wan-shaper"

echo "===== 1. 启动 prod-sim 环境（2c/2g + 生产 GUC + 限流开启）====="
if [ "${SKIP_BUILD:-0}" = "1" ]; then "${COMPOSE[@]}" up -d; else "${COMPOSE[@]}" up -d --build; fi
for i in $(seq 1 60); do
  curl -fs http://localhost:8443/api/v1/health >/dev/null 2>&1 && break
  sleep 2
done
curl -fs http://localhost:8443/api/v1/health >/dev/null
# 深度健康：真实路由须 200（冷启动窗口 health 可过而业务路由尚不可用——实测教训）
for i in $(seq 1 45); do
  curl -fs http://localhost:8443/api/v1/pow/challenge >/dev/null 2>&1 && break
  sleep 2
done
curl -fs http://localhost:8443/api/v1/pow/challenge >/dev/null || {
  echo "[FAIL] pow/challenge 深健康检查 90s 未通过" >&2; exit 1; }
echo "[ok] app 健康（health + pow 深检查通过）"

if [ "${SKIP_SEED:-0}" = "1" ]; then
  echo "===== 2. seed 跳过（SKIP_SEED=1，复用已有数据）====="
elif [ -n "${SEED_DOCKER_IMAGE:-}" ]; then
  # CI/无 Rust 工具链宿主：容器内编译运行（rust toolchain 镜像，host 网络连 15432）
  docker run --rm --network host -e SQLX_OFFLINE=true -v "$REPO_ROOT":/w -w /w/backend \
    "$SEED_DOCKER_IMAGE" \
    cargo run --release -p wakewake-seed -- --count "$SEED_COUNT" --dsn "$PG_DSN"
else
  (cd "$REPO_ROOT/backend" && cargo run --release -p wakewake-seed -- --count "$SEED_COUNT" --dsn "$PG_DSN")
fi
# pairing codes 始终重导（幂等）

echo "===== 3. 导出 pairing codes ====="
docker exec "$PG_CONTAINER" psql -U wakewake -d wakewake_test -t -A \
  -c "SELECT pairing_code FROM agents ORDER BY id" > "$LOAD_DIR/pairing_codes.txt"
CODES=$(wc -l < "$LOAD_DIR/pairing_codes.txt")
echo "[ok] $CODES codes"

echo "===== 4. 出口整形 ====="
apply_shaping

echo "===== 5. 监控采集 ====="
# tc qdisc 统计采样（R3 带宽墙证据：bytes/drops 按 5s 快照，差分得速率）
docker rm -f wakewake-wan-sampler >/dev/null 2>&1 || true
docker run -d --name wakewake-wan-sampler --network "container:$APP_CONTAINER" \
  --cap-add NET_ADMIN \
  --entrypoint sh wakewake-wan-shaper \
  -c 'while true; do date +%s; tc -s qdisc show dev eth0; echo ---; sleep 5; done' >/dev/null
LOAD_APP_CONTAINER=$APP_CONTAINER LOAD_PG_CONTAINER=$PG_CONTAINER \
  LOAD_METRICS_OUT=results/metrics-samples.csv \
  "$LOAD_DIR/collect-metrics.sh" &
COLLECT_PID=$!

echo "===== 6. 运行场景：$SCENARIO ====="

r5_phase() { # r5_phase <sub>  宿主 python(undici fetch 对本 rig 间歇 404,弃用)
  R5_EMAIL="$(cat "$LOAD_DIR/results/r5-email.txt" 2>/dev/null || echo "r5-$(date +%s)@load.wakewake.local")" \
    python3 "$LOAD_DIR/r5-wol-e2e.py" "$1"
}

if [ "$SCENARIO" = "r5-wol-e2e" ]; then
  echo "[R5] 真实 wake 端到端（真实用户 + 真实 agent 容器）"
  email=$(r5_phase prepare | tail -1) && echo "$email" > "$LOAD_DIR/results/r5-email.txt"
  # 邮箱验证:mailer 关闭,DB 直改(e2e.md §4.5 先例)
  docker exec "$PG_CONTAINER" psql -U wakewake -d wakewake_test -c \
    "UPDATE users SET email_verified=true WHERE email='$email'" >/dev/null
  echo "[R5] 已验证 $email"
  code=$(r5_phase getcode | tail -1)
  WAKEWAKE_PAIRING_CODE="$code" "${COMPOSE[@]}" --profile agent up -d agent
  echo "[R5] agent 已启动,开始测量…"
  r5_phase measure | tee "$LOAD_DIR/results/r5-report.md"
else
  # 场景透传变量（各脚本自有默认值，仅传宿主已设置的）
  K6_ENV=(-e BASE_URL=http://app:8443 -e PAIRING_CODES_FILE=/scripts/pairing_codes.txt
    -e SEED_COUNT="$SEED_COUNT")
  for v in CONNS DURATION R2_STAGES R3_RESTART_AFTER R3_HOLD_MS R1_HOLD_MS SOAK_CONNS SOAK_SESS_RATE SESS_RATE SESSION_SECS WAKE_PROB LOGIN_RATE TARGET_CONNS STAIR_STEP PAGE_RATE; do
    if [ -n "${!v:-}" ]; then K6_ENV+=(-e "$v=${!v}"); fi
  done
  docker rm -f wakewake-load-k6 >/dev/null 2>&1 || true
  K6_CPUS_FLAG=()
  if [ -n "${K6_CPUSET:-}" ]; then
    K6_CPUS_FLAG=(--cpuset-cpus "$K6_CPUSET") # loader 钉离 SUT 专核（markpost 先例）
  fi
  docker run --rm --name wakewake-load-k6 --network "$NETWORK" \
    --memory 8g --memory-swap 8g "${K6_CPUS_FLAG[@]}" \
    -v "$LOAD_DIR":/scripts \
    "${K6_ENV[@]}" \
    "$K6_IMAGE" run "/scripts/$SCENARIO.js" &
  K6_PID=$!

  # R3：连接建满后重启 app（netns 重建 → 整形必须重注入）
  if [ "$SCENARIO" = "r3-reconnect-storm" ]; then
    echo "[R3] ${R3_RESTART_AFTER}s 后重启 $APP_CONTAINER（触发全量重连风暴）"
    sleep "$R3_RESTART_AFTER"
    docker restart "$APP_CONTAINER"
    echo "[R3] netns 已重建，重注入整形"
    apply_shaping
    # 重启后 health 恢复前 connect 会 5xx，属预期（风暴窗口）
  fi

  wait $K6_PID || echo "[WARN] k6 非零退出（阈值击穿=容量数据，markpost 先例，流水线继续）"
  echo "[k6 完成]"
fi

echo "===== 7. 停止采集 ====="
kill $COLLECT_PID 2>/dev/null || true
# qdisc 快照（带宽证据）落盘后再销毁 sampler
docker logs wakewake-wan-sampler > "$LOAD_DIR/results/wan-qdisc.log" 2>&1 || true
docker rm -f wakewake-wan-sampler >/dev/null 2>&1 || true
echo "[ok] results/wan-qdisc.log（tbf bytes/drops 按 5s 快照，差分即出口速率）"

echo "===== 8. 回归分析 ====="
if [ -f "$LOAD_DIR/results/metrics-samples.csv" ]; then
  python3 "$LOAD_DIR/analyze-regression.py" \
    "$LOAD_DIR/results/metrics-samples.csv" \
    "$LOAD_DIR/results/report.json" > "$LOAD_DIR/results/regression.md" || true
  echo "[ok] results/regression.md"
fi

# 按场景归档 + manifest 追加（markpost 先例：analyze 可按行 join）
ARCH="$LOAD_DIR/results/$SCENARIO"
mkdir -p "$ARCH"
for f in metrics-samples.csv wan-qdisc.log regression.md ${SCENARIO%%-*}-report.md ${SCENARIO%%-*}-report.json; do
  [ -f "$LOAD_DIR/results/$f" ] && cp "$LOAD_DIR/results/$f" "$ARCH/" || true
done
echo "$(date +%s),$(date -Iseconds),$SCENARIO,SEED=$SEED_COUNT WAN=$WAN_RATE/$WAN_DELAY RL=${LOAD_DISABLE_RATE_LIMIT:-0},results/$SCENARIO" \
  >> "$LOAD_DIR/results/manifest.csv"
echo "[ok] 归档 results/$SCENARIO + manifest.csv"

if [ "${KEEP:-0}" = "1" ]; then
  echo "[keep] 环境保留；追加场景用 KEEP=1 再跑（seed 会重复 INSERT，改 SEED_COUNT 或先 down）"
else
  echo "===== 9. 销毁环境 ====="
  "${COMPOSE[@]}" down -v
fi
