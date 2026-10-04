# WAN 整形容器（prod-sim）：共享 app 容器 netns 执行 tc，免改生产镜像。
# 用法见 run-prod-sim.sh：docker run --rm --network container:<app> wakewake-wan-shaper qdisc ...
FROM alpine:3
RUN apk add --no-cache iproute2
ENTRYPOINT ["tc"]
