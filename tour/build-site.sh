#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
runtime_dir=${1:-"${root_dir}/target/tour-runtime"}
site_dir=${2:-"${root_dir}/target/tour-site"}
if [[ "${runtime_dir}" != /* ]]; then
  runtime_dir="${root_dir}/${runtime_dir}"
fi
if [[ "${site_dir}" != /* ]]; then
  site_dir="${root_dir}/${site_dir}"
fi
case "${site_dir}" in
  ""|/|"${root_dir}")
    echo "Refusing unsafe Tour site directory: ${site_dir}" >&2
    exit 1
    ;;
esac
cd "${root_dir}"
cargo xtask generate-browser-plan \
  --check \
  --artifact-dir "${runtime_dir}" \
  --output "${runtime_dir}/browser-plan.json"
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
for generated in \
  browser-plan.json \
  conduit_web.d.ts \
  conduit_web.js \
  conduit_web_bg.wasm \
  conduit_web_bg.wasm.d.ts; do
  cp "${runtime_dir}/${generated}" "${site_dir}/tour/public/${generated}"
done
git -C "${root_dir}" rev-parse HEAD >"${site_dir}/BUILD_COMMIT"
touch "${site_dir}/.nojekyll"
(
  cd "${site_dir}"
  find . -type f ! -name SHA256SUMS -print0 \
    | sort -z \
    | xargs -0 sha256sum >SHA256SUMS
)
