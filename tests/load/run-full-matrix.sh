#!/bin/bash
# 真机全矩阵驱动（markpost capacity 先例：一次 seed、manifest 记录、preflight、
# 按场景归档；loader 钉离 SUT 专核；tbf+netem 整形）。
#
# 顺序（限流开启的场景在前，R2 需关限流单独跑，soak 最后长跑）：
#   seed 22k → R7 静态 → R3 重连风暴 → R1 SSE 拐点 → R4 仪表盘 → R5 wake 端到端
#   → R2 登录阶梯（限流关）→ R6 浸泡（SOAK 时长，默认 8h）
#
# 用法：bash tests/load/run-full-matrix.sh [soak_duration]
#   bash tests/load/run-full-matrix.sh 2h    # 短浸泡
#   bash tests/load/run-full-matrix.sh 8h    # 过夜全量
set -euo pipefail

LOAD_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$LOAD_DIR"

export K6_CPUSET=${K6_CPUSET:-2-11}        # loader 离开 SUT 专核 0,1
export WAN_DELAY=${WAN_DELAY:-"30ms 5ms"}  # markpost 口径：3mbit + 30ms±5 单跳 RTT
SOAK_DURATION=${1:-8h}
SEED=22000
STAMP=$(date +%m%d-%H%M)
mkdir -p results
LOG="results/matrix-$STAMP.log"
exec > >(tee -a "$LOG") 2>&1

echo "===== 全矩阵 @ $(date -Iseconds) loader=cores[$K6_CPUSET] SUT=cores[0,1] wan=3mbit+$WAN_DELAY seed=$SEED soak=$SOAK_DURATION ====="

run() { # run <name> [envs...]
  local name=$1
  shift
  echo ""
  echo ">>>>>>>>>> [$name] $(date +%H:%M:%S) env: $* <<<<<<<<<<"
  env KEEP=1 SKIP_SEED=${SKIP_SEED_FOR:-0} "$@" SCENARIO="$name" bash run-prod-sim.sh
}

# 1. R7 静态（首次运行：起环境 + seed $SEED）
run r7-static DURATION=5m PAGE_RATE=30 SEED_COUNT=$SEED
SKIP_SEED_FOR=1

# 2. R3 重连风暴（HOLD 90s ≈ 3 心跳 → 稳态连接占比 ~95%；重启在 5min 连接全稳后）
run r3-reconnect-storm CONNS=2000 DURATION=12m R3_RESTART_AFTER=300 R3_HOLD_MS=90000 SEED_COUNT=$SEED

# 3. R1 SSE 拐点（5k→20k 四级阶梯）
run r1-sse-knee TARGET_CONNS=20000 STAIR_STEP=5000 SEED_COUNT=$SEED

# 4. R4 仪表盘混合（限流开启，校准档）
run r4-dashboard-mix SESS_RATE=36 DURATION=10m SESSION_SECS=240 WAKE_PROB=0.3 SEED_COUNT=$SEED

# 5. R5 真实 wake 端到端
run r5-wol-e2e R5_ROUNDS=30 SEED_COUNT=$SEED

# 6. R2 登录阶梯（限流关——硬件天花板；compose 检测 env 变化自动重建 app）
run r2-login-staircase SEED_COUNT=$SEED LOAD_DISABLE_RATE_LIMIT=1

# 7. R6 浸泡（回到限流开启；长跑放最后，结束销毁环境）
echo ""
echo ">>>>>>>>>> [r6-soak] $(date +%H:%M:%S) SOAK=$SOAK_DURATION <<<<<<<<<<"
KEEP=0 SKIP_SEED=1 SOAK_CONNS=${SOAK_CONNS:-5000} SOAK_SESS_RATE=${SOAK_SESS_RATE:-6} \
  DURATION="$SOAK_DURATION" SEED_COUNT=$SEED \
  SCENARIO=r6-soak bash run-prod-sim.sh

echo ""
echo "===== 全矩阵完成 @ $(date -Iseconds)；归档见 results/<scenario>/ 与 results/manifest.csv ====="
