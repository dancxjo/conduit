#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
site_dir="${root_dir}/target/tour-site"
cargo xtask generate-browser-plan --check
rm -rf "${site_dir}"
mkdir -p \
  "${site_dir}/tour" \
  "${site_dir}/browser" \
  "${site_dir}/examples" \
  "${site_dir}/docs" \
  "${site_dir}/spec" \
  "${site_dir}/conformance/c4"
cp "${root_dir}/tour/site-index.html" "${site_dir}/index.html"
cp -R "${root_dir}/tour/public" "${site_dir}/tour/public"
cp -R "${root_dir}/tour/book" "${site_dir}/tour/book"
cp -R "${root_dir}/tour/lessons" "${site_dir}/tour/lessons"
cp -R "${root_dir}/tour/reference-panels" "${site_dir}/tour/reference-panels"
cp "${root_dir}"/examples/*.panel "${site_dir}/examples/"
cp "${root_dir}/docs/cookbook-standard-library.md" "${site_dir}/docs/"
cp "${root_dir}/spec/054-text-format.md" "${site_dir}/spec/"
cp "${root_dir}/spec/061-bounded-filesystem.md" "${site_dir}/spec/"
cp "${root_dir}/conformance/c4/text-format.json" "${site_dir}/conformance/c4/"
cp "${root_dir}/conformance/c4/standard-catalog.json" "${site_dir}/conformance/c4/"
cp "${root_dir}/browser/conduit-browser-host.mjs" "${site_dir}/browser/"
cp "${root_dir}/LICENSE" "${site_dir}/LICENSE"
touch "${site_dir}/.nojekyll"
