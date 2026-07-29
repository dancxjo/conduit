#!/usr/bin/env bash
set -euo pipefail
root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
port=$(python3 -c 'import socket; listener = socket.socket(); listener.bind(("127.0.0.1", 0)); print(listener.getsockname()[1]); listener.close()')
server_log=$(mktemp)
browser_log=$(mktemp)
python3 -m http.server "${port}" --bind 127.0.0.1 --directory "${root_dir}" >"${server_log}" 2>&1 &
server_pid=$!
cleanup() {
  kill "${server_pid}" 2>/dev/null || true
  rm -f "${server_log}" "${browser_log}"
}
trap cleanup EXIT
server_ready=false
for _attempt in {1..20}; do
  if curl --fail --silent \
    "http://127.0.0.1:${port}/browser/conduit-browser-host.test.html" >/dev/null; then
    server_ready=true
    break
  fi
  sleep 0.1
done
if [[ "${server_ready}" != true ]]; then
  tail -20 "${server_log}" >&2
  exit 1
fi
passed=false
for _attempt in {1..3}; do
  if google-chrome \
    --headless=new \
    --no-sandbox \
    --autoplay-policy=no-user-gesture-required \
    --enable-unsafe-webgpu \
    --use-angle=swiftshader \
    --virtual-time-budget=25000 \
    --dump-dom \
    "http://127.0.0.1:${port}/browser/conduit-browser-host.test.html" \
    >"${browser_log}" 2>&1 &&
    grep -q '<pre id="result">ok</pre>' "${browser_log}"; then
    passed=true
    break
  fi
done
if [[ "${passed}" != true ]]; then
  tail -80 "${browser_log}" >&2
  exit 1
fi
