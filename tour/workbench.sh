#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
missing=()
for command_name in cargo rustup wasm-bindgen node; do
  command -v "${command_name}" >/dev/null 2>&1 || missing+=("${command_name}")
done
if ((${#missing[@]} > 0)); then
  echo "Workbench cannot start: install the missing tools (${missing[*]}), including Rust's wasm32-unknown-unknown target, wasm-bindgen-cli, and Node.js." >&2
  exit 1
fi
if ! rustup target list --installed | grep -qx "wasm32-unknown-unknown"; then
  echo "Workbench cannot start: run 'rustup target add wasm32-unknown-unknown', then retry 'just workbench'." >&2
  exit 1
fi

host=${CONDUIT_WORKBENCH_HOST:-127.0.0.1}
port=${CONDUIT_WORKBENCH_PORT:-4173}
bash "${root_dir}/tour/build-artifact.sh"
cd "${root_dir}"
CONDUIT_TOUR_SITE="${root_dir}/target/tour-site" \
CONDUIT_STATIC_HOST="${host}" \
CONDUIT_STATIC_LANDING="/tour/public/workbench.html" \
node browser/static-server.mjs "${port}"
