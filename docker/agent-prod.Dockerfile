# syntax=docker/dockerfile:1
# wakewake-agent 生产镜像（specs/backend/agent-distribution.md）。
# 与 E2E 专用 docker/agent.Dockerfile 的区别：不开 danger-insecure-tls、无 s6/wol-sniffer，
# 单进程 entrypoint（常驻由 --restart unless-stopped 承担）。
# 构建上下文：仓库根（docker build -f docker/agent-prod.Dockerfile .）
# 多架构由 CI 原生 runner 各自构建（x86_64 / aarch64），无交叉编译。

# ============================================================
# Stage 0: builder（rust musl 静态二进制）
# ============================================================
FROM rust:1.95-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
# 拷完整 workspace manifest（agent 依赖 protocol，workspace 完整定义）。
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/crates/ ./crates/
# backend/.cargo/config.toml 强制 mold（WSL 用），alpine 无 mold → 覆盖为空
RUN mkdir -p .cargo && printf '' > .cargo/config.toml
RUN cargo build --release --bin wakewake-agent

# ============================================================
# Stage 1: 运行时（alpine 最小 + ca-certificates）
# ============================================================
FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /src/target/release/wakewake-agent /usr/local/bin/wakewake-agent

# WAKEWAKE_HOME=/data：config.toml + key.pem 持久化（卷 wakewake-agent-data:/data）
ENV WAKEWAKE_HOME=/data
VOLUME /data
ENTRYPOINT ["wakewake-agent"]
