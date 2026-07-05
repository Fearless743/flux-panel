#!/usr/bin/env bash
# e2e_real_traffic.sh：启动 gost_agent + 起一个 echo server + 通过 11 命令 round-trip
# 用于手动端到端验证。

set -euo pipefail

PORT="${PORT:-18888}"
ECHO_PORT="${ECHO_PORT:-19999}"

# Start echo server (Python)
python3 -c "
import socket, threading, sys
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('127.0.0.1', $ECHO_PORT))
s.listen()
def loop():
    while True:
        c, _ = s.accept()
        def handle(c):
            try:
                while True:
                    data = c.recv(4096)
                    if not data: break
                    c.sendall(data)
            finally: c.close()
        threading.Thread(target=handle, args=(c,), daemon=True).start()
threading.Thread(target=loop, daemon=True).start()
import time
while True: time.sleep(60)
" &
ECHO_PID=$!
trap "kill $ECHO_PID 2>/dev/null" EXIT

# Start gost_agent forwarding to echo server
cat >/tmp/gost-test.json <<EOF
{
  "addr": "127.0.0.1:8080",
  "secret": "dev-secret",
  "http": 0,
  "tls": 0,
  "socks": 0,
  "ssl": false
}
EOF

# Use cargo run for testing
cargo build --release --bin flux_agent
RUST_LOG=info ./target/release/flux_agent -c /tmp/gost-test.json &
AGENT_PID=$!
sleep 2

# Add a forward service through WebSocket (requires panel)
echo "gost_agent PID: $AGENT_PID"
echo "Echo server PID: $ECHO_PID (port $ECHO_PORT)"
echo "Send WebSocket commands via your panel UI to add a forward service:"
echo "  handler.forwarder.nodes[0].addr = 127.0.0.1:$ECHO_PORT"
echo "Then test with: nc -v 127.0.0.1 $PORT < /dev/urandom"

echo
echo "Press Ctrl+C to stop"
wait
