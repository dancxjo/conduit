#!/usr/bin/env bash
set -euo pipefail
root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=$(google-chrome --headless=new --no-sandbox --disable-gpu --allow-file-access-from-files --virtual-time-budget=3000 --dump-dom "file://${root_dir}/browser/conduit-browser-host.test.html" 2>&1)
grep -q '<pre id="result">ok</pre>' <<<"${output}"
