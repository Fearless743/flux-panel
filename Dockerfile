# syntax=docker/dockerfile:1
# 单一面板镜像：Vite 前端 + Rust 后端 + Caddy 聚合入口
# 对外只暴露 80；后端仅监听 127.0.0.1:6365
#
# 构建加速要点：
# - 前端固定在 BUILDPLATFORM 编译（静态产物，避免 QEMU 下跑 npm/tsc）
# - Rust 后端按 TARGETPLATFORM 构建（multi-arch 时 arm64 在 QEMU 下编译）
# - npm / cargo registry 使用 BuildKit cache mount
# - 镜像内跳过 tsc（类型检查留给本地/独立 CI），只跑 vite build

# ---------- 前端（与架构无关，固定在 host 平台只编一次） ----------
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend
WORKDIR /src

# 依赖层：仅 lockfile 变更时重装
COPY vite-frontend/package.json vite-frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
	npm ci --legacy-peer-deps --no-audit --no-fund

# 源码层
COPY vite-frontend/ ./

# 版本号只影响本层，不污染 npm ci 缓存
ARG APP_VERSION=dev
# 生产构建：VITE_API_BASE 为空 → 浏览器走同源 /api/v1（由 Caddy 反代）
ENV VITE_API_BASE=
ENV VITE_APP_VERSION=${APP_VERSION}
# 跳过 tsc（冷构建约省 25–35s）；类型检查请本地 npm run build / CI 另跑
RUN npx vite build

# ---------- 后端（Rust/Axum；按目标平台构建，支持 multi-arch） ----------
# 依赖层先编译一次，源码变更时复用
FROM rust:1.98-alpine AS backend
WORKDIR /src

RUN apk add --no-cache build-base

COPY rust-backend/Cargo.toml rust-backend/Cargo.lock ./
RUN mkdir -p src \
	&& printf 'fn main() {}\n' > src/main.rs \
	&& cargo build --release \
	&& rm -rf src

COPY rust-backend/src ./src
COPY rust-backend/migrations ./migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	cargo build --release

# ---------- 运行时：Caddy + 二进制 + 静态资源 ----------
FROM caddy:2.8-alpine
RUN apk add --no-cache ca-certificates tzdata wget \
	&& cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
	&& echo Asia/Shanghai > /etc/timezone

WORKDIR /app

COPY --from=backend /src/target/release/flux-panel-backend /app/rust-backend
COPY --from=frontend /src/dist /srv
COPY docker/Caddyfile /etc/caddy/Caddyfile
COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh /app/rust-backend

# CVE-2026-98432：关闭 Caddy（Go）监听套接字的 MPTCP 默认行为；Rust 后端不涉及 MPTCP
ENV GODEBUG=multipathtcp=0 \
	DB_PATH=/app/data/gost.db \
	LOG_DIR=/app/logs \
	INTERNAL_PORT=6365 \
	PORT=6365

VOLUME ["/app/data", "/app/logs"]
EXPOSE 80

HEALTHCHECK --interval=15s --timeout=5s --retries=5 --start-period=25s \
	CMD wget --no-verbose --tries=1 --spider http://127.0.0.1/flow/test || exit 1

STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/entrypoint.sh"]
