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
