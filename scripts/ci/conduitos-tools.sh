#!/usr/bin/env bash
set -euo pipefail

readonly schema='conduit.ci.conduitos-tool-bundle/v1'
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
readonly script_dir
readonly package_manifest="$script_dir/conduitos-tools-packages.txt"
mapfile -t packages < "$package_manifest"
test "${#packages[@]}" -gt 0 || {
  printf 'conduitos-tool-bundle-refused: package manifest is empty\n' >&2
  exit 1
}

refuse() {
  printf 'conduitos-tool-bundle-refused: %s\n' "$1" >&2
  exit 1
}

require_runner_identity() {
  command -v dpkg >/dev/null || refuse 'dpkg is absent'
  test -n "$(runner_image_os)" || refuse 'runner image OS is absent'
  test -n "$(runner_image_version)" || refuse 'runner image version is absent'
}

runner_image_os() {
  . /etc/os-release
  printf '%s' "$ID"
}

runner_image_version() {
  . /etc/os-release
  printf '%s' "$VERSION_ID"
}

runner_os_release() {
  printf '%s/%s' "$(runner_image_os)" "$(runner_image_version)"
}

write_expected_packages() {
  printf '%s\n' "${packages[@]}"
}

verify_bundle() {
  local bundle=$1
  require_runner_identity
  test -f "$bundle/identity.env" || refuse 'identity.env is absent'
  test -f "$bundle/packages.txt" || refuse 'packages.txt is absent'
  test -f "$bundle/SHA256SUMS" || refuse 'SHA256SUMS is absent'

  # This file is data, not shell input. Parse the four exact fields explicitly.
  local actual_schema actual_image_os actual_image_version actual_os_release
  actual_schema=$(sed -n 's/^schema=//p' "$bundle/identity.env")
  actual_image_os=$(sed -n 's/^image_os=//p' "$bundle/identity.env")
  actual_image_version=$(sed -n 's/^image_version=//p' "$bundle/identity.env")
  actual_os_release=$(sed -n 's/^os_release=//p' "$bundle/identity.env")
  test "$actual_schema" = "$schema" || refuse 'schema mismatch'
  test "$actual_image_os" = "$(runner_image_os)" || refuse 'runner image OS mismatch'
  test "$actual_image_version" = "$(runner_image_version)" || refuse 'runner image version mismatch'
  test "$actual_os_release" = "$(runner_os_release)" \
    || refuse 'runner OS release mismatch'
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
  require_runner_identity
  test "$bundle" = "$PWD/target/conduitos/tool-bundle" \
    || refuse 'preparation path must be target/conduitos/tool-bundle'
  test ! -e "$bundle" || refuse 'preparation path already exists'
  test ! -e "$apt_cache" || refuse 'APT staging path already exists'
  install -d -m 0755 "$bundle/debs"
  install -d -m 0777 "$apt_cache/partial"
  write_expected_packages > "$bundle/packages.txt"
  printf 'schema=%s\nimage_os=%s\nimage_version=%s\nos_release=%s/%s\n' \
    "$schema" "$(runner_image_os)" "$(runner_image_version)" \
    "$(runner_image_os)" "$(runner_image_version)" > "$bundle/identity.env"

  sudo apt-get update
  sudo apt-get install --download-only --reinstall -y --no-install-recommends \
    -o "Dir::Cache::archives=$apt_cache" "${packages[@]}"
  # Admit only completed packages. APT's lock and partial state stay outside
  # the bundle and therefore cannot enter its checked file set.
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
    sudo dpkg --install ./*.deb
    sudo dpkg --audit
  ); then
    refuse 'offline package installation failed'
  fi
  local emulator
  for emulator in \
    qemu-system-aarch64 \
    qemu-system-i386 \
    qemu-system-loongarch64 \
    qemu-system-riscv64 \
    qemu-system-x86_64; do
    command -v "$emulator" >/dev/null \
      || refuse "required emulator is absent after installation: $emulator"
    "$emulator" --version >/dev/null \
      || refuse "required emulator cannot load after installation: $emulator"
  done
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
    test $# -eq 2 || refuse 'usage: conduitos-tools.sh prepare BUNDLE'
    prepare_bundle "$(realpath -m "$2")"
    ;;
  verify)
    test $# -eq 2 || refuse 'usage: conduitos-tools.sh verify BUNDLE'
    verify_bundle "$(realpath -m "$2")"
    ;;
  install)
    test $# -eq 2 || refuse 'usage: conduitos-tools.sh install BUNDLE'
    install_bundle "$(realpath -m "$2")"
    ;;
  cache-status)
    test $# -eq 2 || refuse 'usage: conduitos-tools.sh cache-status BUNDLE'
    cache_status "$(realpath -m "$2")"
    ;;
  *)
    refuse 'expected prepare, verify, install, or cache-status'
    ;;
esac
