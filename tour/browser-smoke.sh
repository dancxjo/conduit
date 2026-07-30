#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
server_log=$(mktemp)
browser_log=$(mktemp)
cleanup() {
  kill "${server_pid}" 2>/dev/null || true
  rm -f "${server_log}" "${browser_log}"
}
trap cleanup EXIT

cargo xtask serve --directory "${root_dir}" --port 0 >"${server_log}" 2>&1 &
server_pid=$!

port=""
for _attempt in $(seq 1 30); do
  if ready_line=$(grep -m1 '^READY:' "${server_log}" 2>/dev/null); then
    port="${ready_line#READY:}"
    break
  fi
  sleep 0.1
done

if [[ -z "${port}" ]]; then
  echo "Failed to start static server" >&2
  cat "${server_log}" >&2
  exit 1
fi

for attempt in $(seq 1 20); do
  if curl --fail --silent "http://127.0.0.1:${port}/tour/public/index.html" >/dev/null; then
    break
  fi
  sleep 0.1
done
curl --fail --silent "http://127.0.0.1:${port}/tour/public/index.html" >/dev/null
for attempt in $(seq 1 3); do
  google-chrome --headless --no-sandbox --disable-gpu --virtual-time-budget=10000 --dump-dom \
    "http://127.0.0.1:${port}/tour/public/index.html?autorun" >"${browser_log}"
  if grep --quiet --fixed-strings "exact dedicated-worker placement" "${browser_log}"; then
    break
  fi
done
grep --fixed-strings "Drag nodes to adjust presentation layout" "${browser_log}"
grep --fixed-strings "exact dedicated-worker placement" "${browser_log}"
grep --fixed-strings "conduit/tour-production-wasm-worker" "${browser_log}"
