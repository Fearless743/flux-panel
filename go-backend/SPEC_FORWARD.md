# Forward 规格

文件：
- internal/repo/forward.go, forward_port.go
- internal/service/forward.go
- internal/handler/forward.go
- 使用 gost.BuildForwardServices / PauseResumePayload / DeleteServicePayload
- Hub.SendMsg

## 服务名
fmt.Sprintf("%d_%d_%d", forwardId, userId, userTunnelIdOr0) + _tcp/_udp
limiter = userTunnel.speedId

## create
隧道启用；checkUserPermissions；写 forward；入口分配端口 get_port；AddService；失败回滚

## list
管理员全量/用户自己；拼 inIp 笛卡尔积

### 分页（高性能多页）
`POST /api/v1/forward/list`（路径不变）：
- 空 body / `{}` → legacy 全量数组（兼容移动端与历史调用）。
- 带 `page/pageSize/tunnelId/status/keyword` 任一字段 → 分页语义，
  返回 `{list, total, page, pageSize}`：
  - `page` 从 1 开始，`pageSize` 默认 20、上限 500（导出循环取页）。
  - 过滤全部下推 SQL：`tunnelId/status` 精确匹配，`keyword` LIKE 匹配
    `name/remote_addr/user_name`（`%_\\` 已转义）。
  - 排序固定 `inx ASC, created_time DESC, id DESC`（与拖拽排序语义一致；
    全 0 inx 时退化为创建时间倒序）。
  - 单次请求恒定约 5 条语句（COUNT + 单页 LIMIT/OFFSET + 隧道/端口/节点各一），
    与总量无关；分页查询走覆盖索引（`idx_forward_user_order` 等，见 schema.sql）。
- 前端 `forward.tsx` 服务端分页 + 分页条（10/20/50/100），搜索防抖 400ms；
  多页或筛选时禁用拖拽排序（inx 为全局排序，子集拖拽会破坏顺序）。

## update
可 changeForwardTunnel；UpdateService；status 强制 1

## changeForwardTunnel
先新端口再删旧；权限按归属用户

## delete / forceDelete
delete 调 Gost；force 只 DB

## pause/resume
PauseService/ResumeService；resume 查流量限额

## batch*
部分成功 R.ok 带文案；全失败 R.err

## diagnose
TcpPing

## 权限文案
当前账号已到期 / 你没有该隧道权限 / 隧道被禁用 / 该隧道权限已到期 /
用户总流量已用完 / 该隧道流量已用完 / 数量上限...

读 AGENTS_SPEC.md 与 gost 包。go build ./...
