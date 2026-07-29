#!/usr/bin/env bash
set -euo pipefail
root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
python3 -m http.server 4186 --bind 127.0.0.1 --directory "${root_dir}" >/tmp/conduit-browser-http.log 2>&1 &
server_pid=$!
trap 'kill "${server_pid}" 2>/dev/null || true' EXIT
server_ready=false
for _attempt in {1..20}; do
  if curl --fail --silent \
    http://127.0.0.1:4186/browser/conduit-browser-host.test.html >/dev/null; then
    server_ready=true
    break
  fi
  sleep 0.1
done
if [[ "${server_ready}" != true ]]; then
  tail -20 /tmp/conduit-browser-http.log >&2
  exit 1
fi
output=$(google-chrome \
  --headless=new \
  --no-sandbox \
  --autoplay-policy=no-user-gesture-required \
  --enable-unsafe-webgpu \
  --use-angle=swiftshader \
  --virtual-time-budget=25000 \
  --dump-dom \
  "http://127.0.0.1:4186/browser/conduit-browser-host.test.html" 2>&1)
grep -q '<pre id="result">ok</pre>' <<<"${output}"
