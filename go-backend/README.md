# flux-panel Go 后端（面板业务实现）

Spring Boot 的 **官方替代实现**，**全兼容现网契约**（`/api/v1`、`/flow`、`/system-info`、SQLite schema、JWT、AES-GCM、Gost 指令）。

> `springboot-backend/` 已弃用，见该目录 `DEPRECATED.md`。  
> **生产部署请使用仓库根目录一体镜像** `ghcr.io/fearless743/flux-panel`（Caddy + 本二进制 + 前端），见根 `Dockerfile`。

## 技术栈

- Gin + sqlx + modernc.org/sqlite
- gorilla/websocket
- robfig/cron（流量重置 / 统计 / WAL checkpoint）

## 本地运行（仅后端）

```bash
export PORT=6365
export DB_PATH=./data/gost.db
export JWT_SECRET=your-secret
go run ./cmd/server
```

健康检查：`GET/POST /flow/test` → `test`

默认管理员：`admin_user` / `admin_user`（密码 MD5 入库，与历史 Java 版一致）

## Docker

### 生产：一体镜像（推荐）

在**仓库根目录**：

```bash
docker build -t ghcr.io/fearless743/flux-panel:latest -f Dockerfile .
docker run -d -p 6366:80 -e JWT_SECRET=secret -v data:/app/data ghcr.io/fearless743/flux-panel:latest
```

- 对外 **80**（Caddy）：静态前端 + 反代 `/api/*`、`/flow/*`、`/system-info*` → 本进程 `127.0.0.1:6365`
- CI tag 推送：`flux-panel` + 兼容标签 `go-backend`（同一镜像）

### 仅后端镜像（开发/对照）

```bash
docker build -t go-backend:dev .
# 暴露 6365
```

## 目录

```
cmd/server/          入口
internal/
  auth/              JWT、MD5
  captcha/           简化验证码
  crypto/            AES-GCM
  db/                schema embed、WAL
  gost/              指令 payload / 命名
  handler/           Gin 路由与 handler
  middleware/        CORS、JWT、Admin
  model/             表结构
  repo/              SQL
  service/           业务
  task/              定时任务
  ws/                WebSocket Hub
migrations/          schema/data 副本（对照用）
```

## 兼容说明

- Header：`Authorization: <token>`（无 Bearer）
- 响应：`{code,msg,ts,data}`，成功 `code=0`
- 节点删除：detach 拓扑，不级联删隧道
- UserTunnel 更新：成功返回 ok（修正 Java 恒 err 的 bug）
