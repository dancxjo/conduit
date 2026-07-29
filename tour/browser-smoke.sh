#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
port=$(python3 -c 'import socket; listener = socket.socket(); listener.bind(("127.0.0.1", 0)); print(listener.getsockname()[1]); listener.close()')
server_log=$(mktemp)
browser_log=$(mktemp)
cleanup() {
  kill "${server_pid}" 2>/dev/null || true
  rm -f "${server_log}" "${browser_log}"
}
trap cleanup EXIT

python3 -m http.server "${port}" --directory "${root_dir}/tour" >"${server_log}" 2>&1 &
server_pid=$!
for attempt in $(seq 1 20); do
  if curl --fail --silent "http://127.0.0.1:${port}/public/index.html" >/dev/null; then
    break
  fi
  sleep 0.1
done
curl --fail --silent "http://127.0.0.1:${port}/public/index.html" >/dev/null
google-chrome --headless --no-sandbox --disable-gpu --virtual-time-budget=3000 --dump-dom \
  "http://127.0.0.1:${port}/public/index.html?autorun" >"${browser_log}"
grep --fixed-strings "Hello from the Tour." "${browser_log}"
grep --fixed-strings "Evidence: 2 nodes, 1 cords conducted." "${browser_log}"
grep --fixed-strings "Select a node to reveal its source" "${browser_log}"
