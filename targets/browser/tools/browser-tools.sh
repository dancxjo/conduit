#!/usr/bin/env bash
set -euo pipefail

readonly schema='conduit.ci.browser-tool-bundle/v1'
readonly container_identity='mcr.microsoft.com/playwright:v1.62.0-noble'
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
readonly script_dir
readonly package_manifest="$script_dir/browser-tools-packages.txt"
mapfile -t packages < "$package_manifest"
test "${#packages[@]}" -gt 0 || {
  printf 'browser-tool-bundle-refused: package manifest is empty\n' >&2
  exit 1
}

refuse() {
  printf 'browser-tool-bundle-refused: %s\n' "$1" >&2
  exit 1
}

write_expected_packages() {
  printf '%s\n' "${packages[@]}"
}

verify_bundle() {
  local bundle=$1
  test -f "$bundle/identity.env" || refuse 'identity.env is absent'
  test -f "$bundle/packages.txt" || refuse 'packages.txt is absent'
  test -f "$bundle/SHA256SUMS" || refuse 'SHA256SUMS is absent'

  local actual_schema actual_container actual_os_release actual_dpkg_status
  actual_schema=$(sed -n 's/^schema=//p' "$bundle/identity.env")
  actual_container=$(sed -n 's/^container=//p' "$bundle/identity.env")
  actual_os_release=$(sed -n 's/^os_release=//p' "$bundle/identity.env")
  actual_dpkg_status=$(sed -n 's/^dpkg_status_sha256=//p' "$bundle/identity.env")
  test "$actual_schema" = "$schema" || refuse 'schema mismatch'
  test "$actual_container" = "$container_identity" || refuse 'container identity mismatch'
  test "$actual_os_release" = "$(. /etc/os-release; printf '%s/%s' "$ID" "$VERSION_ID")" \
    || refuse 'container OS release mismatch'
  test "$actual_dpkg_status" = "$(sha256sum /var/lib/dpkg/status | cut -d' ' -f1)" \
    || refuse 'container package baseline mismatch'
  diff -u <(write_expected_packages) "$bundle/packages.txt" \
    || refuse 'package manifest mismatch'
  test -n "$(find "$bundle/debs" -maxdepth 1 -type f -name '*.deb' -print -quit)" \
    || refuse 'bundle contains no Debian packages'
  (
    cd "$bundle"
    sha256sum --check --strict SHA256SUMS
  ) || refuse 'bundle checksum mismatch'
  diff -u \
    <(cd "$bundle" && find . -type f -printf '%P\n' | LC_ALL=C sort) \
    <({
      printf '%s\n' SHA256SUMS identity.env packages.txt
      sed 's/^[[:xdigit:]]\{64\}  //' "$bundle/SHA256SUMS"
    } | LC_ALL=C sort) \
    || refuse 'bundle file set does not match its checksums'
}

prepare_bundle() {
  local bundle=$1
  local apt_cache="${bundle}.apt-cache"
  test "$bundle" = "$PWD/target/browser-tools/tool-bundle" \
    || refuse 'preparation path must be target/browser-tools/tool-bundle'
  test ! -e "$bundle" || refuse 'preparation path already exists'
  test ! -e "$apt_cache" || refuse 'APT staging path already exists'
  install -d -m 0755 "$bundle/debs"
  install -d -m 0777 "$apt_cache/partial"
  write_expected_packages > "$bundle/packages.txt"
  printf 'schema=%s\ncontainer=%s\nos_release=%s/%s\ndpkg_status_sha256=%s\n' \
    "$schema" "$container_identity" \
    "$(. /etc/os-release; printf '%s' "$ID")" \
    "$(. /etc/os-release; printf '%s' "$VERSION_ID")" \
    "$(sha256sum /var/lib/dpkg/status | cut -d' ' -f1)" > "$bundle/identity.env"

  apt-get update
  apt-get install --download-only --reinstall -y --no-install-recommends \
    -o "Dir::Cache::archives=$apt_cache" "${packages[@]}"
  cp "$apt_cache"/*.deb "$bundle/debs/"
  find "$bundle/debs" -maxdepth 1 -type f -name '*.deb' -printf '%f\n' \
    | while IFS= read -r package; do
        sha256sum "$bundle/debs/$package"
      done \
    | sed "s#  $bundle/#  #" \
    | LC_ALL=C sort > "$bundle/SHA256SUMS"
  verify_bundle "$bundle"
}

install_bundle() {
  local bundle=$1
  verify_bundle "$bundle"
  if ! (
    cd "$bundle/debs"
    dpkg --install ./*.deb
    dpkg --audit
  ); then
    refuse 'offline package installation failed'
  fi
}

cache_status() {
  local bundle=$1
  if test ! -e "$bundle"; then
    printf 'absent\n'
  elif (verify_bundle "$bundle") >&2; then
    printf 'present\n'
  else
    test ! -e "${bundle}.rejected" || refuse 'rejected cache path already exists'
    mv "$bundle" "${bundle}.rejected"
    printf 'rejected\n'
  fi
}

case "${1:-}" in
  prepare)
    test $# -eq 2 || refuse 'usage: browser-tools.sh prepare BUNDLE'
    prepare_bundle "$(realpath -m "$2")"
    ;;
  verify)
    test $# -eq 2 || refuse 'usage: browser-tools.sh verify BUNDLE'
    verify_bundle "$(realpath -m "$2")"
    ;;
  install)
    test $# -eq 2 || refuse 'usage: browser-tools.sh install BUNDLE'
    install_bundle "$(realpath -m "$2")"
    ;;
  cache-status)
    test $# -eq 2 || refuse 'usage: browser-tools.sh cache-status BUNDLE'
    cache_status "$(realpath -m "$2")"
    ;;
  *)
    refuse 'expected prepare, verify, install, or cache-status'
    ;;
esac
