# Flow + 定时任务 规格

文件：
- internal/service/flow.go
- internal/handler/flow.go  // FlowUpload FlowConfig FlowTest(可保留 test)
- internal/task/tasks.go
- internal/repo/flow.go（如需）
- 在 cmd/server/main.go 启动 cron（可小改 main）

## FlowTest
返回纯文本 "test"

## FlowConfig ?secret=
节点 secret 查 node；解密 body；异步 CleanNodeConfigs；返回 "ok"

## FlowUpload ?secret=
解密 JSON 数组 [{n,u,d}]；跳过 web_api
解析 n split "_" 取 forwardId,userId,userTunnelId（前三段）
倍率：d/u *= trafficRatio * tunnel.flow
原子更新 forward/user/user_tunnel 流量（d→in_flow, u→out_flow）
超限 PauseService：用户总流量/到期/status；隧道流量/到期/status
pause 时应用正确 serviceName 每条 forward

## CleanNodeConfigs
services: 跳过 web_api；末段 tls 且 tunnel 不存在 → DeleteService；末段 tcp 且 forward 不存在 → Delete tcp+udp
chains: last 段 tunnelId 不存在 → DeleteChains
limiters: id 不存在 → DeleteLimiters

## Cron
ResetFlow: 5 0 0 * * ? — 按 flow_reset_time 日重置；到期用户/隧道 pause+status0
StatisticsFlow: 0 0 * * * ? — 删 48h 前；每用户增量写 statistics_flow
WAL checkpoint 每 5 分钟

依赖 robfig/cron/v3。读 AGENTS_SPEC.md。go build ./...
