#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
site_dir=${CONDUIT_TOUR_SITE:-"${root_dir}/target/tour-site"}
if [[ ! -f "${site_dir}/tour/public/browser-plan.json" ]]; then
  echo "Tour artifact is missing; run bash tour/build-artifact.sh first" >&2
  exit 1
fi
server_log=$(mktemp)
cleanup() {
  kill "${server_pid}" 2>/dev/null || true
  rm -f "${server_log}"
}
trap cleanup EXIT

CONDUIT_TOUR_SITE="${site_dir}" \
  node "${root_dir}/browser/static-server.mjs" 0 >"${server_log}" 2>&1 &
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

chrome_bin="${CHROME_BIN:-}"
if [[ -z "${chrome_bin}" ]] && command -v google-chrome >/dev/null 2>&1; then
  chrome_bin=$(command -v google-chrome)
fi
CHROME_BIN="${chrome_bin}" node "${root_dir}/tour/browser-smoke.mjs" \
  "http://127.0.0.1:${port}/tour/public/index.html?lesson=welcome.hello-panel&autorun"
