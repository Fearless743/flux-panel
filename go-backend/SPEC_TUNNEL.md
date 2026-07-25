# Tunnel + UserTunnel + detach 规格

文件：
- internal/repo/tunnel.go, chain_tunnel.go, user_tunnel.go
- internal/service/tunnel.go, user_tunnel.go
- internal/handler/tunnel.go
- internal/gost 已有 BuildChainData/BuildChainService

## createTunnel
名称唯一；type2 需出口；节点不重复且在线；分配链/出口端口 getNodePort
落库 tunnel+chain_tunnels；type2 下发 AddChains/AddChainService；失败回滚

## list → TunnelDetailDto
inNodeId/chainNodes/outNodeId 分组

## updateTunnel
可 reconfigureTunnelNodes（传入 inNodeId）；更新 name/flow/trafficRatio/inIp
reconfigure: 规范化拓扑、端口沿用、清旧 gost、DB 替换、push Update优先、migrateForwardServices

## deleteTunnel
有 forward 或 user_tunnel 引用则拒绝删除（提示先删转发、取消用户分配）；无关联时清 gost、chain_tunnel、tunnel。**禁止级联删转发/用户权限**

## diagnoseTunnel
TcpPing 拓扑路径

## UserTunnel assign/list/remove/update/mine
update 时 speed 变化要重建 forward 服务
**注意 Java updateUserTunnel 恒返回 err 是 bug，Go 应返回 ok 成功**

## detachNodeFromTunnels（删除节点用）
预校验剩余入口/出口；清 gost；删 chain_tunnel/forward_port；refresh in_ip；type2 重推；warnings 不阻断

## 端口
解析 node.port "a-b,c"；占用 chain_tunnel.port + forward_port.port

读 Java TunnelServiceImpl 工作区文件（含 uncommitted detach）。
读 AGENTS_SPEC.md。go build ./...
