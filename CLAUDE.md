# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

flux-panel（转发面板）管理多节点 GOST 代理：面板下发隧道/转发/限速配置，节点 agent 执行转发并上报流量。

生产形态为**单一 Docker 镜像** `ghcr.io/fearless743/flux-panel`：Caddy(:80) + Go 后端(内部 :6365) + Vite 静态前端。`springboot-backend/` 已弃用（仅作契约对照），勿再构建/发布 Java 镜像。

默认管理员：`admin_user` / `admin_user`（密码 MD5 入库）。

## 仓库结构（大图）

| 路径 | 角色 |
|------|------|
| `go-backend/` | **面板业务后端**（Gin + sqlx + SQLite + gorilla/websocket） |
| `vite-frontend/` | 管理后台（React 18 + Vite + HeroUI + Tailwind） |
| `go-gost/` | **节点 agent**（基于 go-gost/x，含面板 WS 上报与指令执行） |
| `docker/` | 一体镜像 Caddyfile + entrypoint |
| `Dockerfile` | 前端 build → Go build → Caddy 运行时 |
| `install.sh` | 节点安装/更新（拉 Release 中的 `gost-amd64`/`gost-arm64`） |
| `panel_install.sh` | 面板 Docker Compose 安装（`docker-compose-v4/v6.yml`） |
| `springboot-backend/` | 历史 Java 实现，只读对照 |
| `android-app/`、`ios-app/` | 移动端客户端（与 Web 共用 `/api/v1`） |
| `gost-rs/` | 实验性 Rust 节点（若本地存在；**不参与当前发布**） |

## 运行时架构

```
浏览器 / 移动端 ──► Caddy:80 ──► 静态 /srv
                      │
                      ├─ /api/*        ──► go-backend:6365
                      ├─ /flow/*       ──► go-backend:6365   (节点流量/配置，AES-GCM)
                      └─ /system-info* ──► go-backend:6365   (节点+管理端 WebSocket)

节点 agent (go-gost)
  WS 连面板 /system-info?secret=&type=1&version=&http=&tls=&socks=
  收指令: Add/Update/Delete Service|Chains|Limiters, Pause/Resume, TcpPing, SetProtocol, Upgrade
  HTTP: POST /flow/upload、/flow/config（body AES-GCM，密钥=节点 secret）
```

- **配置权威源是面板 DB**，不是节点本地 `gost.json`。节点上线后 `NodeSyncService.SyncNodeConfig` 异步按 DB 重放 limiter → chain → service → pause。
- 面板 → 节点：`Hub.SendMsg` / `SendMsgTimeout`（requestId 等待，默认 10s；升级 15s）。
- 成功判定：`gost.IsOK(msg)` / `gost.NormalizeOK`。

## 常用命令

### 面板后端（`go-backend/`）

```bash
cd go-backend
export PORT=6365 DB_PATH=./data/gost.db JWT_SECRET=your-secret
go run ./cmd/server

go build ./...                    # 改后端后必须通过
go test ./...                     # 现有单测极少
go test ./internal/crypto/        # 跑单个包
go test ./internal/crypto/ -run TestName
```

健康检查：`GET|POST /flow/test` → 纯文本 `test`。

环境变量：`PORT`（默认 6365）、`DB_PATH`、`JWT_SECRET`、`LOG_DIR`。

### 前端（`vite-frontend/`）

```bash
cd vite-frontend
npm ci --legacy-peer-deps         # 或 npm install；Docker/CI 用 package-lock
npm run dev                       # :3000，开发环境 VITE_API_BASE=http://127.0.0.1:6365
npm run build                     # tsc && vite build
npm run lint                      # eslint --fix
npm run preview
```

生产构建：`VITE_API_BASE=`（空）→ 浏览器走同源 `/api/v1`。版本号 `VITE_APP_VERSION`（CI 注入 tag）。

### 节点 agent（`go-gost/`）

```bash
cd go-gost
# 版本由 -ldflags -X main.version=... 注入（CI 用 tag；未注入时为 dev）
VERSION=2.0.9-beta
CGO_ENABLED=0 go build -ldflags="-s -w -X main.version=${VERSION}" -o gost .
# 交叉编译与 CI 一致：
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -ldflags="-s -w -X main.version=${VERSION}" -o gost-amd64 .
CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build -ldflags="-s -w -X main.version=${VERSION}" -o gost-arm64 .
```

### 一体镜像

```bash
# 仓库根目录
docker build -t ghcr.io/fearless743/flux-panel:dev \
  --build-arg APP_VERSION=dev -f Dockerfile .
docker run -d -p 6366:80 -e JWT_SECRET=secret -v data:/app/data \
  ghcr.io/fearless743/flux-panel:dev
```

CI（`.github/workflows/docker-build.yml`）：推送数字开头 tag（如 `2.0.9-beta`）→ 构建 multi-arch 面板镜像 + 压缩 gost 二进制并上传 Release。兼容镜像标签 `go-backend` 与 `flux-panel` 为**同一镜像**。

## go-backend 分层与契约

```
cmd/server/main.go
internal/
  handler/     # Gin 路由与 HTTP 入口（routes.go 路径禁止改）
  service/     # 业务（含 node_sync、tunnel_detach、forward 批量并发）
  repo/        # SQL
  model/       # 表结构
  ws/hub.go    # 节点/管理端会话、SendMsg
  gost/        # 服务命名 + 指令 payload（与节点对账，命名不可随意改）
  auth/        # JWT、MD5 密码
  crypto/      # AES-GCM（密钥 SHA-256(secret)，与节点兼容）
  middleware/  # CORS、JWT、Admin
  response/    # {code,msg,ts,data}，成功 code=0
  task/        # cron：流量重置、统计、WAL checkpoint
  db/          # schema embed、WAL
```

硬规则（详见 `go-backend/AGENTS_SPEC.md` 与 `SPEC_*.md`）：

1. **禁止改 API 路径**（`handler/routes.go` 与 Java 1:1）。
2. JWT Header：`Authorization: <token>`（**无** `Bearer` 前缀）。
3. 密码：`auth.MD5`；错误文案用中文，尽量与历史 Java 一致。
4. 节点删除：`detachNodeFromTunnels`，**禁止**级联删整条隧道。
5. UserTunnel 更新成功应返回 ok（Java 恒 err 是 bug，勿复现）。
6. 状态约定：Node `1` 在线/`0` 离线；Forward `1` 运行/`0` 暂停；User/UserTunnel/Tunnel `1` 正常/`0` 禁用。
7. 服务名（`internal/gost/names.go`）：
   - 转发：`{forwardId}_{userId}_{userTunnelId}_tcp|_udp`
   - 隧道入口：`{tunnelId}_tls`
   - 链：`chains_{tunnelId}`
8. 改后端后至少 `cd go-backend && go build ./...`。

领域规格（实现前先读对应 SPEC）：

- `SPEC_NODE_WS.md` — 节点 CRUD、WS 注册、安装命令
- `SPEC_TUNNEL.md` — 隧道拓扑、detach、UserTunnel
- `SPEC_FORWARD.md` — 转发增删改、批量、pause/resume
- `SPEC_FLOW_TASK.md` — 流量上报/清理、cron
- `SPEC_SPEED.md` — 限速器
- `SPEC_USER.md` — 用户/验证码/配置/OpenAPI 订阅

## 节点 agent 要点（`go-gost/`）

- 入口：`main.go` / `program.go`；面板集成在 `x/socket/`（`websocket_reporter.go`、`upgrade.go` 等）。
- WS 指令类型：`AddService`/`UpdateService`/`DeleteService`/`PauseService`/`ResumeService`、`AddChains`/`UpdateChains`/`DeleteChains`、`AddLimiters`/`UpdateLimiters`/`DeleteLimiters`、`TcpPing`、`SetProtocol`、`Upgrade`。
- 远程升级：`Upgrade` 异步下载校验、备份替换；`upgrade-apply.sh` 健康检查失败回滚。节点必须先具备带 Upgrade 的二进制后，才能用面板一键升。
- Release 产物名：`gost-amd64` / `gost-arm64`。

## 前端要点（`vite-frontend/`）

- 入口：`src/main.tsx`、`src/App.tsx`；API：`src/api/network.ts`（axios + `Authorization` token）、`src/api/index.ts`。
- 页面：`dashboard` / `forward` / `tunnel` / `node` / `user` / `limit` / `config` / `profile` / `settings`；H5 与桌面双布局。
- 路径别名：`@` → `src`。

## 开发时注意

- 分支：`main` 稳定；`beta` 开发版安装脚本默认拉 beta compose。
- 面板「网站配置 → ip」须填 `主机:映射端口`（默认宿主机 6366→容器 80），节点与浏览器共用该入口。
- 对照旧行为时优先读 `springboot-backend/` 对应 Service，但**新代码只改 `go-backend/` / `vite-frontend/` / `go-gost/`**。
- 不要假设存在完整自动化测试套件；回归以编译 + 契约/SPEC + 真机联调为主。
