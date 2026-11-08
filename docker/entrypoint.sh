#!/bin/sh
set -eu

export PORT="${INTERNAL_PORT:-6365}"
export DB_PATH="${DB_PATH:-/app/data/gost.db}"
export LOG_DIR="${LOG_DIR:-/app/logs}"
# JWT_SECRET 无默认值：未设置或过短则拒绝启动，由编排层显式注入强随机值
if [ -z "${JWT_SECRET:-}" ]; then
	echo "[entrypoint] JWT_SECRET 未设置，拒绝启动" >&2
	exit 1
fi
if [ "${#JWT_SECRET}" -lt 32 ]; then
	echo "[entrypoint] JWT_SECRET 过短（至少 32 字符），拒绝启动" >&2
	exit 1
fi
if [ "$JWT_SECRET" = "flux-panel-dev-secret" ]; then
	echo "[entrypoint] JWT_SECRET 仍为开发默认值，拒绝启动" >&2
	exit 1
fi
# CVE-2026-98432：默认关闭 MPTCP 监听，避免受影响的宿主机内核暴露该攻击面
export GODEBUG="${GODEBUG:-multipathtcp=0}"

mkdir -p "$(dirname "$DB_PATH")" "$LOG_DIR"

BACKEND_PID=""
CADDY_PID=""

cleanup() {
	# 先停 Caddy，再停后端（给 WAL checkpoint 时间）
	if [ -n "${CADDY_PID}" ] && kill -0 "${CADDY_PID}" 2>/dev/null; then
		kill -TERM "${CADDY_PID}" 2>/dev/null || true
		wait "${CADDY_PID}" 2>/dev/null || true
	fi
	if [ -n "${BACKEND_PID}" ] && kill -0 "${BACKEND_PID}" 2>/dev/null; then
		kill -TERM "${BACKEND_PID}" 2>/dev/null || true
		# 最多等 25s 让后端优雅退出
		i=0
		while kill -0 "${BACKEND_PID}" 2>/dev/null && [ "$i" -lt 25 ]; do
			sleep 1
			i=$((i + 1))
		done
		kill -KILL "${BACKEND_PID}" 2>/dev/null || true
		wait "${BACKEND_PID}" 2>/dev/null || true
	fi
}

trap cleanup EXIT INT TERM

echo "[entrypoint] starting rust-backend on :${PORT}"
/app/rust-backend &
BACKEND_PID=$!

# 等待后端就绪
i=0
until wget -q --spider "http://127.0.0.1:${PORT}/flow/test" 2>/dev/null; do
	i=$((i + 1))
	if [ "$i" -ge 60 ]; then
		echo "[entrypoint] rust-backend failed to become ready" >&2
		exit 1
	fi
	# 后端已退出
	if ! kill -0 "${BACKEND_PID}" 2>/dev/null; then
		echo "[entrypoint] rust-backend exited early" >&2
		wait "${BACKEND_PID}" || true
		exit 1
	fi
	sleep 0.5
done
echo "[entrypoint] rust-backend ready"

echo "[entrypoint] starting caddy on :80"
caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &
CADDY_PID=$!

# 任一进程退出则整体退出
while kill -0 "${BACKEND_PID}" 2>/dev/null && kill -0 "${CADDY_PID}" 2>/dev/null; do
	sleep 1
done

if ! kill -0 "${BACKEND_PID}" 2>/dev/null; then
	echo "[entrypoint] rust-backend stopped" >&2
	wait "${BACKEND_PID}" || true
	exit 1
fi
echo "[entrypoint] caddy stopped" >&2
wait "${CADDY_PID}" || true
exit 1
