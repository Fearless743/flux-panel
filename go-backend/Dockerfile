# 多架构构建：buildx 会注入 TARGETARCH
# godebug 指令需要 Go 1.23+（见 go.mod，CVE-2026-98432 缓解）
FROM --platform=$BUILDPLATFORM golang:1.23-alpine AS build
ARG TARGETOS=linux
ARG TARGETARCH
WORKDIR /src
RUN apk add --no-cache git ca-certificates
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -trimpath -ldflags="-s -w" -o /out/go-backend ./cmd/server

FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates wget tzdata \
    && cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
    && echo Asia/Shanghai > /etc/timezone
COPY --from=build /out/go-backend /app/go-backend
# CVE-2026-98432：关闭 Go 1.24+ 监听套接字的 MPTCP 默认行为
ENV GODEBUG=multipathtcp=0
ENV DB_PATH=/app/data/gost.db
ENV LOG_DIR=/app/logs
ENV PORT=6365
EXPOSE 6365
HEALTHCHECK --interval=10s --timeout=5s --retries=5 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:6365/flow/test || exit 1
ENTRYPOINT ["/app/go-backend"]
