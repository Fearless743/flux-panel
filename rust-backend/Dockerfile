# syntax=docker/dockerfile:1
# 独立 Rust 后端镜像（开发/对照；生产一体镜像见仓库根 Dockerfile）
FROM rust:1.98-alpine AS build
WORKDIR /src

RUN apk add --no-cache build-base

COPY Cargo.toml Cargo.lock ./
# 先编译依赖，利用 Docker 层缓存
RUN mkdir -p src \
	&& printf 'fn main() {}\n' > src/main.rs \
	&& cargo build --release \
	&& rm -rf src

COPY src ./src
COPY migrations ./migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	cargo build --release

FROM alpine:3.20
RUN apk add --no-cache ca-certificates tzdata \
	&& cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
	&& echo Asia/Shanghai > /etc/timezone
WORKDIR /app
COPY --from=build /src/target/release/flux-panel-backend /app/rust-backend
ENV DB_PATH=/app/data/gost.db \
	LOG_DIR=/app/logs \
	PORT=6365
VOLUME ["/app/data", "/app/logs"]
EXPOSE 6365
HEALTHCHECK --interval=10s --timeout=5s --retries=5 \
	CMD wget --no-verbose --tries=1 --spider http://localhost:6365/flow/test || exit 1
ENTRYPOINT ["/app/rust-backend"]
