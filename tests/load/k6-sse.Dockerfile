# k6 + xk6-sse：grafana/k6 官方镜像不带 SSE 扩展（v2 的 auto-extension 不支持，
# 见 results/analysis-report.md），按已验证组合本地构建：k6 v1.2.1 + xk6-sse v0.1.12。
# xk6 版本线已到 v1.x（经 Go proxy 核实），取最新稳定版构建。
FROM golang:1.25-alpine AS builder
RUN apk add --no-cache git
RUN GOBIN=/usr/local/bin go install go.k6.io/xk6@v1.4.11
RUN xk6 build v1.2.1 --output /k6 \
    --with github.com/phymbert/xk6-sse@v0.1.12

FROM debian:bookworm-slim
COPY --from=builder /k6 /usr/bin/k6
ENTRYPOINT ["k6"]
