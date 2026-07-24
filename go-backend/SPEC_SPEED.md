# SpeedLimit 规格

文件：
- internal/repo/speed_limit.go
- internal/service/speed_limit.go
- internal/handler/speed_limit.go

## create
校验 tunnel；status=1 save；对 chain_tunnel 每个节点 AddLimiters
speed 下发 MB/s = speed/8 一位小数
失败回滚 DB + DeleteLimiters

## list 全表

## update
UpdateLimiters 链上节点；再 updateById（无完整回滚）

## delete
user_tunnel 引用则拒绝；DeleteLimiters；remove

## tunnels
返回全部隧道列表（下拉）

Limiter payload: gost.LimiterData
读 AGENTS_SPEC.md。go build ./...
