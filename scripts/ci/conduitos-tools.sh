#!/usr/bin/env bash
set -euo pipefail

readonly schema='conduit.ci.conduitos-tool-bundle/v1'
readonly packages=(
  build-essential
  curl
  ovmf-ia32
  qemu-efi-aarch64
  qemu-system-arm
  qemu-system-misc
  qemu-system-x86
  u-boot-qemu
  xorriso
)

refuse() {
  printf 'conduitos-tool-bundle-refused: %s\n' "$1" >&2
  exit 1
}

require_runner_identity() {
  test -n "${ImageOS:-}" || refuse 'ImageOS is absent'
  test -n "${ImageVersion:-}" || refuse 'ImageVersion is absent'
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
  test "$actual_image_os" = "$ImageOS" || refuse 'runner image OS mismatch'
  test "$actual_image_version" = "$ImageVersion" || refuse 'runner image version mismatch'
  test "$actual_os_release" = "$(. /etc/os-release; printf '%s/%s' "$ID" "$VERSION_ID")" \
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
    <(cd "$bundle" && find debs -maxdepth 1 -type f -name '*.deb' -printf '%p\n' | LC_ALL=C sort) \
    <(sed 's/^[[:xdigit:]]\{64\}  //' "$bundle/SHA256SUMS" | LC_ALL=C sort) \
    || refuse 'bundle file set does not match its checksums'
}

prepare_bundle() {
  local bundle=$1
  require_runner_identity
  test "$bundle" = "$PWD/target/conduitos/tool-bundle" \
    || refuse 'preparation path must be target/conduitos/tool-bundle'
  test ! -e "$bundle" || refuse 'preparation path already exists'
  install -d -m 0777 "$bundle/debs"
  write_expected_packages > "$bundle/packages.txt"
  printf 'schema=%s\nimage_os=%s\nimage_version=%s\nos_release=%s/%s\n' \
    "$schema" "$ImageOS" "$ImageVersion" \
    "$(. /etc/os-release; printf '%s' "$ID")" \
    "$(. /etc/os-release; printf '%s' "$VERSION_ID")" > "$bundle/identity.env"

  sudo apt-get update
  sudo apt-get install --download-only --reinstall -y --no-install-recommends \
    -o "Dir::Cache::archives=$bundle/debs" "${packages[@]}"
  # APT creates this empty bookkeeping directory mode-private to _apt. The
  # checked bundle contains no partial files, but artifact/cache walkers must
  # be able to prove that rather than failing before verification.
  sudo chmod 0755 "$bundle/debs/partial"
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
    sudo apt-get install -y --no-download --no-install-recommends ./*.deb
  ); then
    refuse 'offline package installation failed'
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
  *)
    refuse 'expected prepare, verify, or install'
    ;;
esac
