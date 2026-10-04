#!/bin/sh
# WakeWake agent 一键安装（specs/backend/agent-distribution.md）。
#
# 用法（由 server agents 页生成，sh -s -- 传参）：
#   curl -fsSL https://raw.githubusercontent.com/jukanntenn/wakewake/main/install.sh \
#     | sh -s -- --server https://your-wakewake.example.com --code <配对码>
#
# 步骤：平台检测 → 下载 Release 资产（musl 静态）→ sha256 校验 → 安装二进制 →
#       写 ~/.wakewake/config.toml（已有则备份）→ 前台启动。
# 幂等：重复执行覆盖二进制、备份旧配置后重写（配对码轮换后重跑同一行命令即可）。
# 常驻：前台验证成功后 Ctrl+C，运行 sudo wakewake-agent service install。
set -eu

REPO="jukanntenn/wakewake"
BIN="wakewake-agent"

fail() { echo "error: $*" >&2; exit 1; }
usage() {
  cat >&2 <<EOF
usage: install.sh --server <URL> --code <pairing-code>

  --server   wakewake server URL（如 https://wakewake.example.com）
  --code     pairing code（server agents 页获取）
EOF
  exit 1
}

server=""
code=""
while [ $# -gt 0 ]; do
  case "$1" in
    --server) [ $# -ge 2 ] || usage; server="$2"; shift 2 ;;
    --server=*) server="${1#*=}"; shift ;;
    --code) [ $# -ge 2 ] || usage; code="$2"; shift 2 ;;
    --code=*) code="${1#*=}"; shift ;;
    -h|--help) usage ;;
    *) fail "unknown argument: $1" ;;
  esac
done
[ -n "$server" ] || { echo "error: --server is required" >&2; usage; }
[ -n "$code" ] || { echo "error: --code is required" >&2; usage; }

# ---- 平台检测（仅 Linux；Windows/macOS 见 README planned）----
[ "$(uname -s)" = "Linux" ] || fail "unsupported OS: $(uname -s)（当前仅支持 Linux）"
case "$(uname -m)" in
  x86_64|amd64) target="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) target="aarch64-unknown-linux-musl" ;;
  armv7l|armv8l) target="armv7-unknown-linux-musleabihf" ;;
  *) fail "unsupported arch: $(uname -m)（支持 x86_64 / aarch64 / armv7；armv6 如 Pi 1/Zero 暂不支持）" ;;
esac

# ---- 依赖检查 ----
for cmd in curl tar; do
  command -v "$cmd" >/dev/null 2>&1 || fail "missing dependency: $cmd"
done

# ---- 下载 + sha256 校验（手工比较，不依赖 sha256sum -c 的 busybox 差异）----
archive="$BIN-$target.tar.gz"
base="https://github.com/$REPO/releases/latest/download"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo ">> downloading $archive"
curl -fsSL -o "$tmpdir/$archive" "$base/$archive" || fail "download failed（尚无 release？见 README 的源码编译路径）"
curl -fsSL -o "$tmpdir/$archive.sha256" "$base/$archive.sha256" || fail "checksum download failed"

expected="$(awk 'NR==1 {print $1}' "$tmpdir/$archive.sha256")"
actual="$(sha256sum "$tmpdir/$archive" 2>/dev/null | awk '{print $1}')"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  fail "sha256 mismatch（下载损坏或被篡改，请重试）"
fi

# ---- 解压（taiki-e archive 根级二进制；防御性回退子目录）----
tar -xzf "$tmpdir/$archive" -C "$tmpdir"
if [ -f "$tmpdir/$BIN" ]; then
  bin_path="$tmpdir/$BIN"
elif [ -f "$tmpdir/$target/$BIN" ]; then
  bin_path="$tmpdir/$target/$BIN"
else
  fail "archive layout unexpected（未找到 $BIN）"
fi

# ---- 安装二进制（root → /usr/local/bin；否则 ~/.local/bin + PATH 提示）----
if [ "$(id -u)" = "0" ] || [ -w /usr/local/bin ]; then
  dest="/usr/local/bin"
else
  dest="$HOME/.local/bin"
  mkdir -p "$dest"
  case ":${PATH}:" in
    *":$dest:"*) ;;
    *) echo ">> note: $dest 不在 PATH，请将其加入 shell 配置" ;;
  esac
fi
install -m 0755 "$bin_path" "$dest/$BIN"
echo ">> installed $dest/$BIN"

# ---- 写配置（已有则备份；key.pem 永不触碰；含配对码 → 0600）----
home="${WAKEWAKE_HOME:-$HOME/.wakewake}"
cfg="$home/config.toml"
mkdir -p "$home"
if [ -f "$cfg" ]; then
  backup="$cfg.bak.$(date +%Y%m%d%H%M%S)"
  cp "$cfg" "$backup"
  echo ">> backed up existing config → $backup"
fi
umask 077
cat > "$cfg" <<EOF
# 由 install.sh 生成（历史配置备份于同目录 *.bak.*）
server_url = "$server"
pairing_code = "$code"
EOF
chmod 600 "$cfg"
echo ">> wrote $cfg"

# ---- 前台启动（连接成功后 Ctrl+C；常驻：sudo wakewake-agent service install）----
echo ">> starting agent（前台运行；常驻运行请执行：sudo $BIN service install）"
exec "$dest/$BIN" --config "$cfg"
