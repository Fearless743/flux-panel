# go-backend 并行实现契约（子代理必读）

## 仓库路径
- 项目根：`/home/fearless/github/flux-panel`
- Go 后端：`/home/fearless/github/flux-panel/go-backend`
- Java 对照：`/home/fearless/github/flux-panel/springboot-backend`

## 模块路径
```
go-backend/
  cmd/server/main.go          # 已有，尽量不改签名
  internal/handler/           # App 方法：替换 stubs.go 中对应桩
  internal/service/           # 新建业务逻辑
  internal/repo/              # 新建 SQL
  internal/ws/hub.go          # 已有 Hub.SendMsg
  internal/gost/              # 已有命名与 payload
  internal/auth/              # JWT + MD5
  internal/crypto/            # AES-GCM
  internal/model/             # 表结构
  internal/response/          # R 响应
  internal/middleware/        # JWT/Admin/CORS
  internal/db/                # SQLite
```

## 硬规则
1. **禁止改 API 路径**（见 `handler/routes.go`）
2. 响应：`response.OK/Err/Unauthorized`，字段 `code/msg/ts/data`
3. JWT Header：`Authorization` 无 Bearer
4. 密码：`auth.MD5`
5. 节点指令：`app.Hub.SendMsg(nodeID, data, type)`，成功判定 `gost.IsOK(msg)` / `gost.NormalizeOK`
6. 服务名：`gost.ForwardServiceBase` / `TunnelTLS` / `ChainName`
7. SQLite 经 `app.DB` (*sqlx.DB)，写操作用事务
8. 只改你负责的文件；共享包只**增加**函数，不改已有公开签名
9. 完成后必须 `cd go-backend && go build ./...` 通过
10. 用中文错误文案，与 Java 一致

## App 依赖
```go
type App struct {
  DB *sqlx.DB
  JWT *auth.JWT
  Hub *ws.Hub
  Config config.Config
}
```

## JSON 字段
前端多为 camelCase（role_id 登录返回例外用 role_id）。登录返回：
```json
{"token":"...","name":"...","role_id":0,"requirePasswordChange":false}
```

## 状态
- Node status: 1在线 0离线
- Forward status: 1运行 0暂停
- User/UserTunnel/Tunnel status: 1正常 0禁用

## 删除节点
必须实现 `detachNodeFromTunnels`，禁止级联删整条隧道。
