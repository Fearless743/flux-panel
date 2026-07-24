# syntax=docker/dockerfile:1
# 单一面板镜像：Vite 前端 + Go 后端 + Caddy 聚合入口
# 对外只暴露 80；后端仅监听 127.0.0.1:6365

# ---------- 前端 ----------
FROM node:20-alpine AS frontend
WORKDIR /src
COPY vite-frontend/package.json vite-frontend/package-lock.json ./
RUN npm ci --legacy-peer-deps
COPY vite-frontend/ ./
# 生产构建：VITE_API_BASE 为空 → 浏览器走同源 /api/v1（由 Caddy 反代）
ENV VITE_API_BASE=
RUN npm run build

# ---------- 后端 ----------
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS backend
ARG TARGETOS=linux
ARG TARGETARCH
WORKDIR /src
RUN apk add --no-cache git ca-certificates
COPY go-backend/go.mod go-backend/go.sum ./
RUN go mod download
COPY go-backend/ ./
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
	go build -trimpath -ldflags="-s -w" -o /out/go-backend ./cmd/server

# ---------- 运行时：Caddy + 二进制 + 静态资源 ----------
FROM caddy:2.8-alpine
RUN apk add --no-cache ca-certificates tzdata wget \
	&& cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
	&& echo Asia/Shanghai > /etc/timezone

WORKDIR /app

COPY --from=backend /out/go-backend /app/go-backend
COPY --from=frontend /src/dist /srv
COPY docker/Caddyfile /etc/caddy/Caddyfile
COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh /app/go-backend

ENV DB_PATH=/app/data/gost.db \
	LOG_DIR=/app/logs \
	INTERNAL_PORT=6365 \
	PORT=6365

VOLUME ["/app/data", "/app/logs"]
EXPOSE 80

HEALTHCHECK --interval=15s --timeout=5s --retries=5 --start-period=25s \
	CMD wget --no-verbose --tries=1 --spider http://127.0.0.1/flow/test || exit 1

STOPSIGNAL SIGTERM
ENTRYPOINT ["/app/entrypoint.sh"]
