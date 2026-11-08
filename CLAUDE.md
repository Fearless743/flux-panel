# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

flux-panel（转发面板）管理多节点 GOST 代理：面板下发隧道/转发/限速配置，节点 agent 执行转发并上报流量。

生产形态为**单一 Docker 镜像** `ghcr.io/fearless743/flux-panel`：Caddy(:80) + Rust 后端(内部 :6365) + Vite 静态前端。工作树仅保留 `rust-backend/` 生产后端；历史 Go/Java 实现已归档到分支 `archive/go-backend` / `archive/springboot-backend`，勿再作为生产入口。

默认管理员：`admin_user` / `admin_user`（密码 MD5 入库）。

## 仓库结构（大图）

| 路径 | 角色 |
|------|------|
| `rust-backend/` | **面板业务后端（生产）**（Axum + Tokio + sqlx + SQLite） |
| `vite-frontend/` | 管理后台（React 18 + Vite + HeroUI + Tailwind） |
| `go-gost/` | **节点 agent**（基于 go-gost/x，含面板 WS 上报与指令执行） |
| `docker/` | 一体镜像 Caddyfile + entrypoint |
| `Dockerfile` | 前端 build → Rust build → Caddy 运行时 |
| `install.sh` | 节点安装/更新（拉 Release 中的 `gost-amd64`/`gost-arm64`） |
| `panel_install.sh` | 面板 Docker Compose 安装（`docker-compose-v4/v6.yml`） |

移动端套壳（WebView）已归档到分支 `archive/mobile-apps`；移动浏览器可直接用前端 H5 布局。

历史后端对照请看归档分支：`archive/go-backend`（Go）、`archive/springboot-backend`（Java），例如 `git show archive/go-backend:go-backend/internal/config/config.go`。

## 运行时架构

```
浏览器 / 移动端 ──► Caddy:80 ──► 静态 /srv
                      │
                      ├─ /api/*        ──► rust-backend:6365
                      ├─ /flow/*       ──► rust-backend:6365   (节点流量/配置，AES-GCM)
                      └─ /system-info* ──► rust-backend:6365   (节点+管理端 WebSocket)

节点 agent (go-gost)
  WS 连面板 /system-info?secret=&type=1&version=&http=&tls=&socks=
  收指令: Add/Update/Delete Service|Limiters, Pause/Resume, TcpPing, SetProtocol, Upgrade
  HTTP: POST /flow/upload、/flow/config（body AES-GCM，密钥=节点 secret）
```

- **配置权威源是面板 DB**，不是节点本地 `gost.json`。节点上线后 `NodeSyncService.SyncNodeConfig` 异步按 DB 重放 limiter → service → pause。
- 面板 → 节点：`Hub.SendMsg` / `SendMsgTimeout`（requestId 等待，默认 10s；升级 15s）。
- 成功判定：`gost.IsOK(msg)` / `gost.NormalizeOK`。

## 常用命令

### 面板后端（`rust-backend/`）

```bash
cd rust-backend
export PORT=6365 DB_PATH=./data/gost.db JWT_SECRET=your-secret
cargo run

cargo check                       # 改后端后必须通过
cargo test                        # 单元测试
./scripts/parity.sh               # Go/Rust 契约对照（启动两个后端）
```

健康检查：`GET|POST /flow/test` → 纯文本 `test`。

环境变量：`PORT`（默认 6365）、`DB_PATH`、`JWT_SECRET`、`LOG_DIR`。

历史 Go 实现（仅对照/回滚，见归档分支 `archive/go-backend`）：`git show archive/go-backend:go-backend/cmd/server/main.go`。

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

CI（`.github/workflows/docker-build.yml`）：推送数字开头 tag（如 `2.0.9-beta`）→ 构建 multi-arch 面板镜像 + 压缩 gost 二进制并上传 Release。面板镜像统一为 `flux-panel`。

## rust-backend 分层与契约

```
src/
  main.rs       # 入口、路由装配、任务启动
  handler/      # Axum 路由与 HTTP 入口（mod.rs 路径禁止改）
  service/      # 业务（含 node_sync、tunnel_detach、forward 批量并发）
  repo/         # SQL（与 Go 版逐字一致）
  model.rs      # 表结构 / serde JSON
  ws/hub.rs     # 节点/管理端会话、send_msg
  gost.rs       # 服务命名 + 指令 payload（与节点对账，命名不可随意改）
  auth/         # JWT、MD5 密码
  crypto.rs     # AES-GCM（密钥 SHA-256(secret)，与节点兼容）
  middleware.rs # CORS、JWT、Admin
  response.rs   # {code,msg,ts,data}，成功 code=0
  task/         # 流量重置、统计、WAL checkpoint
  db.rs         # schema embed、WAL
```

硬规则（详见 `rust-backend/PORTING.md`；Go 版 SPEC 已随 `archive/go-backend` 归档）：

1. **禁止改 API 路径**（`src/handler/mod.rs` 与 Java 1:1）。
2. JWT Header：`Authorization: <token>`（**无** `Bearer` 前缀）。
3. 密码：`auth::md5_hex`；错误文案用中文，尽量与历史 Java 一致。
4. 节点删除：`service::tunnel_detach::detach_node_from_tunnels`，**禁止**级联删整条隧道。
5. UserTunnel 更新成功应返回 ok（Java 恒 err 是 bug，勿复现）。
6. 状态约定：Node `1` 在线/`0` 离线；Forward `1` 运行/`0` 暂停；User/UserTunnel/Tunnel `1` 正常/`0` 禁用。
7. 服务名（`src/gost.rs`）：
   - 转发：`{forwardId}_{userId}_{userTunnelId}_tcp|_udp`
   - 隧道入口：`{tunnelId}_tls`
8. 改后端后至少 `cd rust-backend && cargo check`；契约变更跑 `./scripts/parity.sh`。

领域规格（实现前先读对应 SPEC）：

- `SPEC_NODE_WS.md` — 节点 CRUD、WS 注册、安装命令
- `SPEC_TUNNEL.md` — 隧道拓扑、detach、UserTunnel
- `SPEC_FORWARD.md` — 转发增删改、批量、pause/resume
- `SPEC_FLOW_TASK.md` — 流量上报/清理、cron
- `SPEC_SPEED.md` — 限速器
- `SPEC_USER.md` — 用户/验证码/配置/OpenAPI 订阅

## 节点 agent 要点（`go-gost/`）

- 入口：`main.go` / `program.go`；面板集成在 `x/socket/`（`websocket_reporter.go`、`upgrade.go` 等）。
- WS 指令类型：`AddService`/`UpdateService`/`DeleteService`/`PauseService`/`ResumeService`、`AddLimiters`/`UpdateLimiters`/`DeleteLimiters`、`TcpPing`、`SetProtocol`、`Upgrade`。
- 远程升级：`Upgrade` 异步下载校验、备份替换；`upgrade-apply.sh` 健康检查失败回滚。节点必须先具备带 Upgrade 的二进制后，才能用面板一键升。
- Release 产物名：`gost-amd64` / `gost-arm64`。
- 内存调优（`main.go` → `internal/memtune`，`tune` 有单测）：
  - `GOST_GOMEMLIMIT`：显式 Go 软内存上限（如 `512MiB`/`1G`）。未设置时自动读取 cgroup 内存上限并取其 90%（Docker/systemd 部署均生效），避免默认 GOGC=100 让堆涨到存活量 2 倍、小内存节点 RSS 偏高甚至 OOM。
  - `GOST_GOGC`：覆盖 GOGC（默认 100；`off` 表示完全依赖 GOMEMLIMIT）。
  - `GOST_LOG_LEVEL`：日志级别（默认 info）。连接生命周期日志（relay handler 的 `<>`/`><`）默认走 Debug，减小每连接的日志分配；需要按连接排查时设为 `info`。
  - 热路径已用 `bufpool`（TCP 中转 2×64KB/连接复用）；流量经 `GlobalTrafficManager` 按 5s 批量上报。

## 前端要点（`vite-frontend/`）

- 入口：`src/main.tsx`、`src/App.tsx`；API：`src/api/network.ts`（axios + `Authorization` token）、`src/api/index.ts`。
- 页面：`dashboard` / `forward` / `tunnel` / `node` / `user` / `limit` / `config` / `profile` / `settings`；H5 与桌面双布局。
- 路径别名：`@` → `src`。

## 开发时注意

- 分支：`beta` 为默认/主分支（`main` 已删除）；历史后端版本见归档分支 `archive/go-backend`、`archive/springboot-backend`。
- 面板「网站配置 → ip」须填 `主机:映射端口`（默认宿主机 6366→容器 80），节点与浏览器共用该入口。
- 对照旧行为时优先读归档分支（`git show archive/go-backend:...` / `git show archive/springboot-backend:...`）对应 Service，但**新代码只改 `rust-backend/` / `vite-frontend/` / `go-gost/`**。
- 不要假设存在完整自动化测试套件；回归以编译 + 契约/SPEC + 真机联调为主。
