# Node + WebSocket 实现规格

文件：
- internal/repo/node.go
- internal/service/node.go
- internal/handler/node.go
- internal/handler/websocket.go  // HandleWebSocket
- 可扩展 internal/ws/hub.go（只增不改签名）

## NodeCreate
name/serverIp/port 必填；validatePortRange（逗号/区间）；secret=uuid 无横线；status=0

## NodeList
ORDER BY status DESC；返回前 secret 置空

## NodeUpdate
在线且 http/tls/socks 变化 → Hub.SendMsg SetProtocol；失败返回 gost msg；再 updateById

## NodeDelete
1. detachNodeFromTunnels — **若 tunnel service 尚未实现，先写接口调用 service.Tunnel.DetachNodeFromTunnels，可在 service/tunnel_detach.go 实现 detach 核心**
2. remove node

## NodeInstall
读 vite_config ip；无 → 请先前往网站配置中设置ip
命令 curl install.sh -a <ip> -s <secret>；ssl=true 加 -l
IPv6 用 gost.ProcessServerAddress

## WebSocket /system-info
Query: secret, type, version, http, tls, socks
type=="1": 按 secret 查 node，RegisterNode，status=1 更新 version/http/tls/socks，广播 {"id","type":"status","data":1}
否则 JWT 校验 secret 字段，RegisterAdmin
读消息循环：Hub.HandleIncoming；节点消息广播 admin {"id","type":"info","data"}
关闭：节点若仍是当前 session → status=0 广播 data:0

读 AGENTS_SPEC.md 与 internal/ws/hub.go。完成后 go build ./...
