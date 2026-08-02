#!/usr/bin/env bash
set -euo pipefail
root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
runtime_dir=${1:-"${root_dir}/target/tour-runtime"}
if [[ "${runtime_dir}" != /* ]]; then
  runtime_dir="${root_dir}/${runtime_dir}"
fi
mkdir -p "${runtime_dir}"
cd "${root_dir}"
cargo build -p conduit-web --target wasm32-unknown-unknown --release
wasm-bindgen \
  --target web \
  --out-dir "${runtime_dir}" \
  target/wasm32-unknown-unknown/release/conduit_web.wasm
node browser/check-wasm-bridge.mjs "${runtime_dir}"
cargo xtask generate-browser-plan \
  --artifact-dir "${runtime_dir}" \
  --output "${runtime_dir}/browser-plan.json"
