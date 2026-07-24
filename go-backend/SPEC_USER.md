# User / Captcha / Config / OpenAPI 实现规格

实现文件（仅这些）：
- internal/repo/user.go, config.go
- internal/service/user.go, config.go, captcha.go
- internal/handler/user.go, captcha.go, config.go, openapi.go
- internal/captcha/captcha.go

## UserLogin POST /api/v1/user/login
Body: {username, password, captchaId?}
1. vite_config captcha_enabled=="true" 时必须 captchaId 且通过二次校验
2. SELECT user WHERE user=?
3. 密码 auth.MD5 比对；status==0 → 账号被停用
4. 返回 data: {token, name, role_id, requirePasswordChange}
   requirePasswordChange = username=="admin_user" || password=="admin_user"

## UserCreate (admin)
user/pwd/flow/num/expTime/flowResetTime；role_id=1 status=1；用户名唯一

## UserList (admin)
role_id <> 0 全部用户

## UserUpdate (admin)
不可改 role_id=0；用户名唯一；pwd 空不更新

## UserDelete (admin)
不可删管理员；先删其 forward（调 forward 删除若无则先删 DB forward/ports）、user_tunnel、statistics_flow、user

## UserPackage
当前用户套餐：userInfo + tunnelPermissions(SQL联表) + forwards(联表+拼 inIp) + statisticsFlows 24h

## UserUpdatePassword
校验当前密码、确认密码、用户名冲突

## UserReset (admin)
type==1 清 user 流量；否则清 user_tunnel 流量

## Captcha
check: captcha_enabled → data 0/1
generate: 简化算术/字符图 base64 + captchaId
verify: 校验后标记 captchaId 可用于 login secondary

## Config
get(public): {name} → 实体
list: Map name→value
update map / update-single upsert

## OpenAPI GET /open_api/sub_store?user&pwd&tunnel=-1
MD5 鉴权；header subscription-userinfo: upload=inFlow; download=outFlow; total=flow*GIGA; expire=exp/1000
注意 Java buildSubscriptionHeader 参数交叉：upload 头放 inFlow，download 放 outFlow
成功 body 为 header 字符串

读 AGENTS_SPEC.md。完成后 go build ./...
