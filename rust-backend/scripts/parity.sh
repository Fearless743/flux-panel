#!/usr/bin/env bash
# Go / Rust 后端契约对照：同一份 schema、同一组请求，比较规范化后的 JSON。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP="$(mktemp -d)"
GO_PORT=16365
RS_PORT=16366
GO_PID=""
RS_PID=""
WS_PIDS=()
FAIL=0

cleanup() {
  for p in "${WS_PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  [[ -n "$GO_PID" ]] && kill "$GO_PID" 2>/dev/null || true
  [[ -n "$RS_PID" ]] && kill "$RS_PID" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

echo "== build =="
(cd "$ROOT/go-backend" && go build -o "$TMP/go-backend" ./cmd/server)
(cd "$ROOT/rust-backend" && cargo build --quiet)
cp "$ROOT/rust-backend/target/debug/flux-panel-backend" "$TMP/rust-backend"

start() {
  local bin="$1" port="$2" db="$3"
  PORT="$port" DB_PATH="$db" JWT_SECRET=parity-secret "$bin" >"$TMP/${port}.log" 2>&1 &
  echo $!
}

GO_PID="$(start "$TMP/go-backend" "$GO_PORT" "$TMP/go.db")"
RS_PID="$(start "$TMP/rust-backend" "$RS_PORT" "$TMP/rust.db")"

for port in "$GO_PORT" "$RS_PORT"; do
  for _ in $(seq 1 100); do
    if curl -fsS "http://127.0.0.1:${port}/flow/test" >/dev/null 2>&1; then break; fi
    sleep 0.1
  done
done

post() {
  local port="$1" path="$2" body="$3" token="${4:-}"
  local args=(-sS -X POST "http://127.0.0.1:${port}${path}" -H 'Content-Type: application/json')
  [[ -n "$token" ]] && args+=(-H "Authorization: ${token}")
  if [[ -n "$body" ]]; then args+=(--data "$body"); fi
  curl "${args[@]}"
}

norm() {
  jq -S 'walk(if type == "object"
              then del(.ts, .time, .createdTime, .updatedTime, .timestamp, .exportedAt)
              | if has("secret") then .secret = "<secret>" else . end
              else . end)
        | if ((.data | type) == "object") and (.data | has("token"))
          then .data.token = "<token>" else . end'
}

compare() {
  local name="$1" go_json="$2" rs_json="$3"
  local go_norm rs_norm
  go_norm="$(printf '%s' "$go_json" | norm 2>/dev/null || printf '%s' "$go_json")"
  rs_norm="$(printf '%s' "$rs_json" | norm 2>/dev/null || printf '%s' "$rs_json")"
  if [[ "$go_norm" == "$rs_norm" ]]; then
    echo "PASS  $name"
  else
    echo "FAIL  $name"
    diff -u <(printf '%s\n' "$go_norm") <(printf '%s\n' "$rs_norm") || true
    FAIL=1
  fi
}

# 公共接口
compare "login" \
  "$(post "$GO_PORT" /api/v1/user/login '{"username":"admin_user","password":"admin_user"}')" \
  "$(post "$RS_PORT" /api/v1/user/login '{"username":"admin_user","password":"admin_user"}')"
compare "captcha/check" \
  "$(post "$GO_PORT" /api/v1/captcha/check '{}')" \
  "$(post "$RS_PORT" /api/v1/captcha/check '{}')"
compare "config/get" \
  "$(post "$GO_PORT" /api/v1/config/get '{"name":"app_name"}')" \
  "$(post "$RS_PORT" /api/v1/config/get '{"name":"app_name"}')"

GO_TOKEN="$(post "$GO_PORT" /api/v1/user/login '{"username":"admin_user","password":"admin_user"}' | jq -r .data.token)"
RS_TOKEN="$(post "$RS_PORT" /api/v1/user/login '{"username":"admin_user","password":"admin_user"}' | jq -r .data.token)"

compare "user/package" \
  "$(post "$GO_PORT" /api/v1/user/package '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/user/package '{}' "$RS_TOKEN")"
compare "user/list" \
  "$(post "$GO_PORT" /api/v1/user/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/user/list '{}' "$RS_TOKEN")"
compare "node/list" \
  "$(post "$GO_PORT" /api/v1/node/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/node/list '{}' "$RS_TOKEN")"
compare "tunnel/list" \
  "$(post "$GO_PORT" /api/v1/tunnel/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/tunnel/list '{}' "$RS_TOKEN")"
compare "speed-limit/list" \
  "$(post "$GO_PORT" /api/v1/speed-limit/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/speed-limit/list '{}' "$RS_TOKEN")"
compare "config/list" \
  "$(post "$GO_PORT" /api/v1/config/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/config/list '{}' "$RS_TOKEN")"
compare "forward/list(empty body)" \
  "$(post "$GO_PORT" /api/v1/forward/list '' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/forward/list '' "$RS_TOKEN")"
compare "forward/list({})" \
  "$(post "$GO_PORT" /api/v1/forward/list '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/forward/list '{}' "$RS_TOKEN")"
compare "backup/export" \
  "$(post "$GO_PORT" /api/v1/backup/export '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/backup/export '{}' "$RS_TOKEN")"

GO_SUB="$(curl -sS "http://127.0.0.1:${GO_PORT}/api/v1/open_api/sub_store?user=admin_user&pwd=admin_user")"
RS_SUB="$(curl -sS "http://127.0.0.1:${RS_PORT}/api/v1/open_api/sub_store?user=admin_user&pwd=admin_user")"
if [[ "$GO_SUB" == "$RS_SUB" ]]; then echo "PASS  open_api/sub_store"; else
  echo "FAIL  open_api/sub_store"; diff -u <(printf '%s\n' "$GO_SUB") <(printf '%s\n' "$RS_SUB") || true; FAIL=1
fi

# ---- 写操作场景 ----
cmp_admin() {
  local name="$1" path="$2" body="$3"
  compare "$name" \
    "$(post "$GO_PORT" "$path" "$body" "$GO_TOKEN")" \
    "$(post "$RS_PORT" "$path" "$body" "$RS_TOKEN")"
}

cmp_admin node/create /api/v1/node/create '{"name":"n1","serverIp":"127.0.0.1","port":"20000-20010","tcpListenAddr":"[::]","udpListenAddr":"[::]"}'
cmp_admin node/list /api/v1/node/list '{}'
cmp_admin config/update-single /api/v1/config/update-single '{"name":"ip","value":"1.2.3.4"}'
cmp_admin config/get /api/v1/config/get '{"name":"ip"}'

GO_INST="$(post "$GO_PORT" /api/v1/node/install '{"id":1}' "$GO_TOKEN" | sed -E 's/ -s [0-9a-f]{32}/ -s SECRET/')"
RS_INST="$(post "$RS_PORT" /api/v1/node/install '{"id":1}' "$RS_TOKEN" | sed -E 's/ -s [0-9a-f]{32}/ -s SECRET/')"
compare "node/install" "$GO_INST" "$RS_INST"

cmp_admin node/update /api/v1/node/update '{"id":1,"name":"n1b","serverIp":"127.0.0.1","port":"20000-20010","tcpListenAddr":"[::]","udpListenAddr":"[::]","http":0,"tls":0,"socks":0}'
cmp_admin node/list /api/v1/node/list '{}'

# WebSocket 节点上线/离线（不主动下发命令，仅验证 /system-info 注册与状态广播）
start_ws_node() {
  local port="$1" secret="$2" tag="$3"
  local marker="$TMP/ws-${tag}.open"
  node -e '
    const fs = require("fs");
    const ws = new WebSocket(process.argv[1]);
    ws.onopen = () => fs.writeFileSync(process.argv[2], "open");
    ws.onmessage = () => {};
    setInterval(() => {}, 1000);
  ' "ws://127.0.0.1:${port}/system-info?secret=${secret}&type=1&version=v9&http=1&tls=1&socks=1" "$marker" &
  WS_PIDS+=("$!")
  for _ in $(seq 1 100); do [[ -f "$marker" ]] && break; sleep 0.05; done
}

GO_SECRET="$(sqlite3 "$TMP/go.db" "SELECT secret FROM node WHERE id=1")"
RS_SECRET="$(sqlite3 "$TMP/rust.db" "SELECT secret FROM node WHERE id=1")"
start_ws_node "$GO_PORT" "$GO_SECRET" go
start_ws_node "$RS_PORT" "$RS_SECRET" rs
sleep 0.3
cmp_admin node/list /api/v1/node/list '{}'
for p in "${WS_PIDS[@]}"; do kill "$p" 2>/dev/null || true; done
WS_PIDS=()
sleep 0.5
cmp_admin node/list /api/v1/node/list '{}'

# 让节点在线（Tunnel create 要求 status=1），两边同样处理
sqlite3 "$TMP/go.db" "PRAGMA busy_timeout=5000; UPDATE node SET status=1 WHERE id=1;"
sqlite3 "$TMP/rust.db" "PRAGMA busy_timeout=5000; UPDATE node SET status=1 WHERE id=1;"

cmp_admin tunnel/create /api/v1/tunnel/create '{"name":"t1","type":1,"flow":1,"trafficRatio":1.0,"inIp":"","inNodeId":[{"nodeId":1}],"chainNodes":[],"outNodeId":[]}'
cmp_admin tunnel/list /api/v1/tunnel/list '{}'
cmp_admin user/create /api/v1/user/create '{"user":"u1","pwd":"p1","flow":100,"num":10,"expTime":2727251700000,"flowResetTime":1,"status":1}'
cmp_admin user/list /api/v1/user/list '{}'
cmp_admin tunnel/user/assign /api/v1/tunnel/user/assign '{"userId":2,"tunnelId":1,"flow":100,"num":10,"flowResetTime":1,"expTime":2727251700000}'
cmp_admin tunnel/user/list /api/v1/tunnel/user/list '{"userId":2}'

# 直接落一条 forward + forward_port（避免依赖在线节点的 WS AddService）
for db in "$TMP/go.db" "$TMP/rust.db"; do
  sqlite3 "$db" "PRAGMA busy_timeout=5000;
    INSERT INTO forward (id,user_id,user_name,name,tunnel_id,remote_addr,strategy,in_flow,out_flow,created_time,updated_time,status,inx)
      VALUES (1,2,'u1','f1',1,'example.com:80','fifo',0,0,1000,1000,1,0);
    INSERT INTO forward_port (id,forward_id,node_id,port) VALUES (1,1,1,20000);"
done

GO_U1_TOKEN="$(post "$GO_PORT" /api/v1/user/login '{"username":"u1","password":"p1"}' | jq -r .data.token)"
RS_U1_TOKEN="$(post "$RS_PORT" /api/v1/user/login '{"username":"u1","password":"p1"}' | jq -r .data.token)"

cmp_u1() {
  local name="$1" path="$2" body="$3"
  compare "$name" \
    "$(post "$GO_PORT" "$path" "$body" "$GO_U1_TOKEN")" \
    "$(post "$RS_PORT" "$path" "$body" "$RS_U1_TOKEN")"
}

compare "u1/package" \
  "$(post "$GO_PORT" /api/v1/user/package '{}' "$GO_U1_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/user/package '{}' "$RS_U1_TOKEN")"
cmp_u1 "u1/forward/list" /api/v1/forward/list '{}'
cmp_u1 "u1/forward/list(paged)" /api/v1/forward/list '{"page":1,"pageSize":10,"keyword":"f1"}'
cmp_u1 "u1/forward/create(offline node)" /api/v1/forward/create '{"name":"f2","tunnelId":1,"remoteAddr":"example.com:80","strategy":"fifo"}'

# FlowUpload：明文上报，验证 forward/user/user_tunnel 原子累加与 package 统计
GO_SECRET="$(sqlite3 "$TMP/go.db" "SELECT secret FROM node WHERE id=1")"
RS_SECRET="$(sqlite3 "$TMP/rust.db" "SELECT secret FROM node WHERE id=1")"
compare "flow/upload(plain)" \
  "$(post "$GO_PORT" "/flow/upload?secret=${GO_SECRET}" '[{"n":"1_2_1_tcp","u":100,"d":200}]')" \
  "$(post "$RS_PORT" "/flow/upload?secret=${RS_SECRET}" '[{"n":"1_2_1_tcp","u":100,"d":200}]')"
compare "u1/package(after flow)" \
  "$(post "$GO_PORT" /api/v1/user/package '{}' "$GO_U1_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/user/package '{}' "$RS_U1_TOKEN")"
cmp_u1 "u1/forward/list(after flow)" /api/v1/forward/list '{}'

cmp_u1 "u1/forward/pause" /api/v1/forward/pause '{"id":1}'
cmp_u1 "u1/forward/resume" /api/v1/forward/resume '{"id":1}'
cmp_u1 "u1/forward/update-order" /api/v1/forward/update-order '{"forwards":[{"id":1,"inx":5}]}'
cmp_u1 "u1/forward/delete" /api/v1/forward/delete '{"id":1}'
cmp_u1 "u1/forward/list(final)" /api/v1/forward/list '{}'
cmp_admin speed-limit/create /api/v1/speed-limit/create '{"name":"sl1","speed":80,"tunnelId":1,"tunnelName":"t1"}'
cmp_admin backup/export /api/v1/backup/export '{}'

# 交叉导入：Go 备份 → Rust、Rust 备份 → Go，再比较导出结果
GO_EXPORT="$(post "$GO_PORT" /api/v1/backup/export '{}' "$GO_TOKEN")"
RS_EXPORT="$(post "$RS_PORT" /api/v1/backup/export '{}' "$RS_TOKEN")"
GO_IMPORT="$(post "$GO_PORT" /api/v1/backup/import "$RS_EXPORT" "$GO_TOKEN")"
RS_IMPORT="$(post "$RS_PORT" /api/v1/backup/import "$GO_EXPORT" "$RS_TOKEN")"
compare "backup/import(cross)" "$GO_IMPORT" "$RS_IMPORT"
compare "backup/export(after cross import)" \
  "$(post "$GO_PORT" /api/v1/backup/export '{}' "$GO_TOKEN")" \
  "$(post "$RS_PORT" /api/v1/backup/export '{}' "$RS_TOKEN")"
cmp_admin tunnel/delete /api/v1/tunnel/delete '{"id":1}'
cmp_admin user/delete /api/v1/user/delete '{"id":2}'
cmp_admin tunnel/delete /api/v1/tunnel/delete '{"id":1}'
cmp_admin node/delete /api/v1/node/delete '{"id":1}'
cmp_admin node/list /api/v1/node/list '{}'
cmp_admin tunnel/list /api/v1/tunnel/list '{}'
cmp_admin user/list /api/v1/user/list '{}'

# type=2 隧道：节点在线但无 WS 连接，验证下发失败后的整体回滚
cmp_admin node/create /api/v1/node/create '{"name":"n2","serverIp":"127.0.0.2","port":"20020-20030","tcpListenAddr":"[::]","udpListenAddr":"[::]"}'
cmp_admin node/create /api/v1/node/create '{"name":"n3","serverIp":"127.0.0.3","port":"20040-20050","tcpListenAddr":"[::]","udpListenAddr":"[::]"}'
for db in "$TMP/go.db" "$TMP/rust.db"; do
  sqlite3 "$db" "PRAGMA busy_timeout=5000; UPDATE node SET status=1 WHERE id IN (2,3);"
done
cmp_admin tunnel/create2 /api/v1/tunnel/create '{"name":"t2","type":2,"flow":1,"trafficRatio":1.0,"inIp":"","inNodeId":[{"nodeId":2}],"chainNodes":[],"outNodeId":[{"nodeId":3}]}'
cmp_admin tunnel/list2 /api/v1/tunnel/list '{}'
cmp_admin node/delete2 /api/v1/node/delete '{"id":2}'
cmp_admin node/delete3 /api/v1/node/delete '{"id":3}'
cmp_admin node/list2 /api/v1/node/list '{}'

# CORS：OPTIONS 预检状态码与关键响应头
GO_CORS="$(curl -sS -D - -o /dev/null -X OPTIONS "http://127.0.0.1:${GO_PORT}/api/v1/user/login" | tr -d '\r' | grep -iE '^(HTTP/|access-control-allow-origin|access-control-allow-methods|access-control-allow-headers|access-control-expose-headers)' | tr '[:upper:]' '[:lower:]' | sort)"
RS_CORS="$(curl -sS -D - -o /dev/null -X OPTIONS "http://127.0.0.1:${RS_PORT}/api/v1/user/login" | tr -d '\r' | grep -iE '^(HTTP/|access-control-allow-origin|access-control-allow-methods|access-control-allow-headers|access-control-expose-headers)' | tr '[:upper:]' '[:lower:]' | sort)"
if [[ "$GO_CORS" == "$RS_CORS" ]]; then echo "PASS  cors/options"; else
  echo "FAIL  cors/options"; diff -u <(printf '%s\n' "$GO_CORS") <(printf '%s\n' "$RS_CORS") || true; FAIL=1
fi

exit "$FAIL"
