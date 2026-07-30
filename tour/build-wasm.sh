#!/usr/bin/env bash
set -euo pipefail
root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "${root_dir}"
cargo build -p conduit-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir tour/public target/wasm32-unknown-unknown/release/conduit_web.wasm
cargo xtask generate-browser-plan
