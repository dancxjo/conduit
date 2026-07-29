#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
site_dir="${root_dir}/target/tour-site"
python3 "${root_dir}/tour/generate-browser-plan.py" --check
rm -rf "${site_dir}"
mkdir -p "${site_dir}/tour" "${site_dir}/browser"
cp "${root_dir}/tour/site-index.html" "${site_dir}/index.html"
cp -R "${root_dir}/tour/public" "${site_dir}/tour/public"
cp -R "${root_dir}/tour/lessons" "${site_dir}/tour/lessons"
cp "${root_dir}/browser/conduit-browser-host.mjs" "${site_dir}/browser/"
cp "${root_dir}/LICENSE" "${site_dir}/LICENSE"
touch "${site_dir}/.nojekyll"
