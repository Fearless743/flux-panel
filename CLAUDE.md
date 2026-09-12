# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

flux-panel（转发面板）管理多节点 GOST 代理：面板下发隧道/转发/限速配置，节点 agent 执行转发并上报流量。

生产形态为**单一 Docker 镜像** `ghcr.io/fearless743/flux-panel`：Caddy(:80) + Go 后端(内部 :6365) + Vite 静态前端。`springboot-backend/` 已弃用（仅作契约对照），勿再构建/发布 Java 镜像。

默认管理员：`admin_user` / `admin_user`（密码 MD5 入库）。

## 仓库结构（大图）

| 路径 | 角色 |
| ------ | ------ |
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

```text
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
go vet ./...                      # 改后端后建议一并跑
# 单测极少（crypto/names/ws 三个包），跑全部或指定：
go test ./...                     
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

CI（`.github/workflows/docker-build.yml`）：推送数字开头 tag（如 `2.0.9-beta`）→ 构建 multi-arch 面板镜像 + 压缩 gost 二进制并上传 Release。

## go-backend 分层、契约与约定

### 分层结构（文本示意）

```
cmd/server/main.go          # 启动：加载配置 → 开 DB → 建 Hub → 注册路由 → 起 cron
internal/
  handler/                  # Gin 路由入口（App 结构体持有 DB/JWT/Hub/Config）
  service/                  # 业务逻辑（每模块一个 .go，函数命名 PascalCase）
  repo/                     # 数据访问（每表一个 *Repo 结构体 + NewXxxRepo(db) 构造函数）
  model/                    # DB 表结构（struct tag：db:"column" json:"field"）
  ws/hub.go                 # WebSocket 会话管理（节点+管理端双通道）
  gost/                     # 服务命名规则 + 指令 payload 工具函数（与节点对账）
  auth/                     # JWT 签发/验证、MD5 密码
  crypto/                   # AES-GCM 加密（密钥 = SHA-256(secret)）
  middleware/               # CORS、JWT、Admin；ctx key：CtxUserID/CtxRoleID/CtxName
  response/                 # R{} 响应包（OK/Err/Unauthorized/ErrCode）
  task/                     # cron 定时任务（流量重置/统计/WAL checkpoint）
  db/                       # SQLite 初始化（embed schema.sql + data.sql）
```

### Go 代码约定

- **import 顺序**：标准库 → 第三方（按字母序）→ 项目内（`github.com/Fearless743/flux-panel/...`），组间空一行。
- **错误处理**：统一 `fmt.Errorf("xxx: %w", err)` 包装；判断空行用 `errors.Is(err, sql.ErrNoRows)`。
- **repo 模式**：每个表对应一个 `XxxRepo` 结构体，构造函数 `NewXxxRepo(db *sqlx.DB)`，方法直接操作 DB。
- **service 模式**：构造函数签名 `NewXxxService(db *sqlx.DB, hub *ws.Hub)`，持有 `DB`、`Hub`、`Repo` 字段；业务函数 PascalCase（如 `CreateForward`）。批量推送节点时并发度固定为 `batchConcurrency = 8`。
- **response 响应**：所有 HTTP 接口返回 `{code,msg,ts,data}`，成功 `code=0`，失败 `code=-1`，未登录 `code=401`，无权限 `code=403`。用 `response.OK/Err/Unauthorized/Forbidden(c, ...)` 辅助函数，勿手动构造 `gin.Context.JSON`。
- **model 标签**：struct tag 同时含 `db:"column"` 和 `json:"field"`（驼峰），如 `db:"id" json:"id"`。
- **DB 连接**：`modernc.org/sqlite`（纯 Go，无 CGO）；WAL 模式，`MaxOpenConns=1`（写串行防竞争）。
- **配置嵌入**：`//go:embed schema.sql data.sql` 在启动时自动执行，无需外部迁移工具。
- **日志**：标准库 `log/slog`，Info 级别输出到 stdout；无结构化 JSON 日志。

### 硬规则

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

---

## 前端约定（`vite-frontend/`）

- **TypeScript 严格模式**：`strict: true`、`noUnusedLocals`、`noUnusedParameters`、`noFallthroughCasesInSwitch` 均开启，编译时必须干净。
- **ESLint**：用 flat config（`eslint.config.mjs`），`npm run lint` 已带 `--fix`；import 顺序有分组约束（type → builtin → external → internal → parent → sibling → index）。
- **路径别名**：`@/*` → `src/*`（tsconfig + vite 均配置）。
- **API 客户端**：`src/api/network.ts` 用 axios，拦截器处理 401（清 localStorage → 跳首页）；生产环境走同源 `/api/v1/`（Caddy 反代）。
- **构建**：Docker 中跳过 `tsc`（只跑 `vite build`），类型检查留给本地或 CI；开发环境 Vite :3000，`VITE_API_BASE` 指向后端地址。
- **UI 组件库**：统一用 **HeroUI**（`@heroui/*`），页面内按需引入（`Button`、`Input`、`Modal`、`Chip`、`Select`、`RadioGroup`、`DatePicker`、`Spinner`、`Progress`、`Divider` 等），勿自行写基础控件。
- **Toast**：用 `react-hot-toast`，操作反馈调用 `toast.success()/toast.error()`。
- **类型定义**：前端业务类型集中在 `src/types/index.ts`，后端请求/响应结构体也需在此补充对应 interface，避免散落在页面文件中。
- **页面结构**：`src/pages/` 下每模块一个 `.tsx`，组件以 function component + hooks 为主；图标放 `src/components/icons.tsx`。

## 节点 agent 要点（`go-gost/`）

- 入口：`main.go` / `program.go`；面板集成在 `x/socket/`（`websocket_reporter.go`、`upgrade.go` 等）。
- WS 指令类型：`AddService`/`UpdateService`/`DeleteService`/`PauseService`/`ResumeService`、`AddChains`/`UpdateChains`/`DeleteChains`、`AddLimiters`/`UpdateLimiters`/`DeleteLimiters`、`TcpPing`、`SetProtocol`、`Upgrade`。
- 远程升级：`Upgrade` 异步下载校验、备份替换；`upgrade-apply.sh` 健康检查失败回滚。节点必须先具备带 Upgrade 的二进制后，才能用面板一键升。
- Release 产物名：`gost-amd64` / `gost-arm64`。

## 开发时注意

- 分支：`main` 稳定；`beta` 开发版安装脚本默认拉 beta compose。
- 面板「网站配置 → ip」须填 `主机:映射端口`（默认宿主机 6366→容器 80），节点与浏览器共用该入口。
- 对照旧行为时优先读 `springboot-backend/` 对应 Service，但**新代码只改 `go-backend/` / `vite-frontend/` / `go-gost/`**。
- 不要假设存在完整自动化测试套件；回归以编译 + 契约/SPEC + 真机联调为主。
