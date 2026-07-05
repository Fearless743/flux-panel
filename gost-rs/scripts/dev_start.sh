#!/usr/bin/env bash
# dev_start.sh：在本地起一个临时 gost_agent 实例（dev 模式）。
# 用法：./scripts/dev_start.sh [path-to-config.json]

set -euo pipefail

BIN="${BIN:-./target/release/flux_agent}"
if [[ ! -x "$BIN" ]]; then
  echo "Building release binary..."
  cargo build --release --bin flux_agent
fi

CONFIG="${1:-./config.json}"
if [[ ! -f "$CONFIG" ]]; then
  cat >"$CONFIG" <<EOF
{
  "addr": "127.0.0.1:8080",
  "secret": "dev-secret",
  "http": 0,
  "tls": 0,
  "socks": 0,
  "ssl": false
}
EOF
  echo "Created default $CONFIG"
fi

if [[ ! -f "gost.json" ]]; then
  cat >gost.json <<'EOF'
{
  "services": [],
  "chains": [],
  "hops": []
}
EOF
  echo "Created default gost.json"
fi

echo "Starting $BIN with $CONFIG"
exec "$BIN" -c "$CONFIG"
