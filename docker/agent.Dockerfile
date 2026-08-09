# syntax=docker/dockerfile:1
# wakewake-agent 镜像（e2e.md §6.1）。
# E2E 专用构建：启用 danger-insecure-tls feature（信任 Caddy tls internal 自签名）。
# s6 管理 agent + UDP WoL sniffer 两进程（e2e.md §6.3）。
#
# 构建上下文：仓库根（docker build -f docker/agent.Dockerfile .）
# （与 app.Dockerfile 一致，docker/agent/s6/ 在仓库根下）

# ============================================================
# Stage 0: builder（rust musl 静态二进制）
# ============================================================
FROM rust:1.95-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
# 拷完整 workspace manifest（agent 依赖 protocol，workspace 完整定义）。
# server/seed 源也拷（manifest 引用为成员，但 --bin wakewake-agent 不编它们）。
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/crates/ ./crates/
# backend/.cargo/config.toml 强制 mold（WSL 用），alpine 无 mold → 覆盖为空
RUN mkdir -p .cargo && printf '' > .cargo/config.toml
# E2E 专用：启用 danger-insecure-tls feature（e2e.md §6.2）
# 仅编 agent 二进制（server/seed 源不影响 agent 编译）
RUN cargo build --release --features danger-insecure-tls --bin wakewake-agent

# ============================================================
# Stage 1: 运行时（alpine + s6 + socat 用于 WoL sniffer）
# ============================================================
FROM alpine:3.21
RUN apk add --no-cache socat busybox-extras ca-certificates xz wget tcpdump
COPY --from=builder /src/target/release/wakewake-agent /usr/local/bin/wakewake-agent

# s6 服务定义（agent longrun + wol-sniffer longrun）
COPY docker/agent/s6/s6-rc.d /etc/s6-overlay/s6-rc.d
COPY docker/agent/s6/user-bundles.d /etc/s6-overlay/user-bundles.d

ARG S6_OVERLAY_VERSION=3.2.3.2
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz /tmp/
RUN S6_ARCH=$(case "$(uname -m)" in x86_64) echo x86_64 ;; aarch64) echo aarch64 ;; esac) && \
    wget -q -O /tmp/s6.tar.xz "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${S6_ARCH}.tar.xz" && \
    tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz && \
    tar -C / -Jxpf /tmp/s6.tar.xz && \
    rm /tmp/*.tar.xz && \
    chmod +x /etc/s6-overlay/s6-rc.d/*/run /etc/s6-overlay/s6-rc.d/*/finish 2>/dev/null; \
    # v3.2.3.2：s6-rc.d/ 下的预装 user/user2 bundle 与 user-bundles.d/ 冲突，
    # 导致 user bundle 不被 legacy-services 启动。删预装让 s6 用 user-bundles.d/。
    rm -rf /etc/s6-overlay/s6-rc.d/user /etc/s6-overlay/s6-rc.d/user2

ENV WAKEWAKE_HOME=/data
VOLUME /data
ENTRYPOINT ["/init"]
