#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
runtime_dir="${root_dir}/target/tour-runtime"
site_dir="${root_dir}/target/tour-site"
dist_dir="${root_dir}/target/tour-dist"

bash "${root_dir}/tour/build-wasm.sh" "${runtime_dir}"
bash "${root_dir}/tour/build-site.sh" "${runtime_dir}" "${site_dir}"

mkdir -p "${dist_dir}"
archive="${dist_dir}/conduit-tour.tar.gz"
source_date_epoch=$(git -C "${root_dir}" show -s --format=%ct HEAD)
(
  cd "${site_dir}"
  tar \
    --sort=name \
    --mtime="@${source_date_epoch}" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -cf - .
) | gzip -n -9 >"${archive}"
(
  cd "${dist_dir}"
  sha256sum conduit-tour.tar.gz >conduit-tour.tar.gz.sha256
)
