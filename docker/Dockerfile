# syntax=docker/dockerfile:1
# wakewake all-in-one 镜像（e2e.md §2.3）。
# 4 阶段：frontend → cargo-chef planner → builder → runtime（s6 + caddy + libpq5）。
#
# 构建上下文必须是仓库根（docker build -f docker/app.Dockerfile .），
# 因为需要同时 COPY backend/ 和 frontend/。

# ============================================================
# Stage 0: 前端构建（Next.js static export → frontend/out/）
# ============================================================
FROM node:22-alpine AS frontend
WORKDIR /web
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
# CI=true 避免 pnpm depsStatusCheck 因 no TTY 报 ERR_PNPM_ABORTED_REMOVE_MODULES_DIR
ENV CI=true
RUN corepack enable && pnpm install --frozen-lockfile
COPY frontend/ .
ENV NEXT_PUBLIC_API_BASE_URL=/api/v1
# Next.js static export → frontend/out/（Caddy file_server 托管）
RUN pnpm build

# ============================================================
# Stage 1: cargo-chef planner（依赖指纹，缓存层）
# ============================================================
FROM rust:1.95-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY backend/ .
RUN cargo chef prepare --recipe-path recipe.json

# ============================================================
# Stage 2: builder（glibc 动态链接 libpq，rustls 替代 openssl）
# ============================================================
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# build-essential 含 gcc/ld/binutils（rust:bookworm 不带 linker，libc build script 需 ld）
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential libpq-dev pkg-config && rm -rf /var/lib/apt/lists/*
# backend/.cargo/config.toml 强制 mold linker（WSL 加速用），Docker 内无 mold。
# 写空 config 覆盖 + 显式清 RUSTFLAGS，强制用默认 ld（Docker 原生 Linux 无 WSL 慢链接问题）。
RUN mkdir -p /app/.cargo && printf '' > /app/.cargo/config.toml
ENV RUSTFLAGS=""
RUN cargo chef cook --release --recipe-path recipe.json
COPY backend/ .
RUN printf '' > /app/.cargo/config.toml
# SQLX_OFFLINE=true 读 backend/.sqlx（无编译期宏，但保留 env 防 sqlx 启动期检查）
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin wakewake-server

# ============================================================
# Stage 3: 运行时（debian-slim + s6-overlay v3.2.3.2 + caddy v2.9.1 + libpq5）
# ============================================================
FROM debian:bookworm-slim
ARG TARGETARCH
ARG S6_OVERLAY_VERSION=3.2.3.2
ARG CADDY_VERSION=v2.9.1
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libpq5 curl tzdata xz-utils && rm -rf /var/lib/apt/lists/*

# s6-overlay v3.2.3.2（noarch + arch-specific，e2e.md §2.2）
# v3.2.3.2 用户 bundle 定义路径迁移到 /etc/s6-overlay/user-bundles.d/（CHANGELOG:12-25）
ADD https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz /tmp/
RUN S6_ARCH=$(case "$TARGETARCH" in amd64) echo x86_64 ;; arm64) echo aarch64 ;; esac) && \
    curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${S6_ARCH}.tar.xz" -o /tmp/s6.tar.xz && \
    tar -C / -Jxpf /tmp/s6-overlay-noarch.tar.xz && \
    tar -C / -Jxpf /tmp/s6.tar.xz && \
    rm /tmp/*.tar.xz

# Caddy 二进制（手动下载，不依赖官方镜像以便 s6 接管，e2e.md §2.3）
RUN curl -fsSL "https://github.com/caddyserver/caddy/releases/download/${CADDY_VERSION}/caddy_${CADDY_VERSION##v}_linux_${TARGETARCH}.tar.gz" \
    | tar xz -C /usr/local/bin caddy && chmod +x /usr/local/bin/caddy

# s6 服务定义（v3.2.3.2 新路径 user-bundles.d）
COPY docker/s6/cont-init.d /etc/cont-init.d
COPY docker/s6/user-bundles.d /etc/s6-overlay/user-bundles.d
COPY docker/s6/s6-rc.d /etc/s6-overlay/s6-rc.d
# 烤进默认 Caddyfile（plain HTTP）；compose 可卷挂载覆盖（验收用 Caddyfile.local，
# e2e 用 e2e/Caddyfile，生产/远程 dev 用 ansible 渲染的 Caddyfile）。
COPY docker/Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/target/release/wakewake-server /app/wakewake-server
COPY --from=frontend /web/out /app/frontend/out
RUN mkdir -p /app/data && chmod +x /etc/cont-init.d/*.sh /etc/s6-overlay/s6-rc.d/*/run

ENV RUST_LOG=info
EXPOSE 8443
WORKDIR /app
ENTRYPOINT ["/init"]
