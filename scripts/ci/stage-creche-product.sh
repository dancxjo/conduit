#!/bin/sh
set -eu

runtime=${1:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
destination=${2:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
release_artifacts=${3:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination/artifacts" "$destination/targets/rp2040/browser-deployment" "$destination/targets/esp32/browser-deployment"

cp targets/browser/host/assets/creche.html "$destination/index.html"
cp targets/browser/host/assets/creche.css "$destination/creche.css"
cp targets/browser/host/assets/creche.mjs "$destination/creche.mjs"
cp targets/browser/host/assets/creche-lifecycle.mjs "$destination/creche-lifecycle.mjs"
cp targets/browser/host/assets/creche-physical.mjs "$destination/creche-physical.mjs"
cp targets/browser/host/assets/creche-target-catalog.mjs "$destination/creche-target-catalog.mjs"
cp targets/browser/host/assets/creche-spore-bundle.mjs "$destination/creche-spore-bundle.mjs"
cp targets/browser/host/assets/creche-graduation.mjs "$destination/creche-graduation.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp targets/browser/host/assets/device-base.mjs "$destination/device-base.mjs"
cp targets/browser/host/assets/usb-device-base.mjs "$destination/usb-device-base.mjs"
cp "$runtime" "$destination/runtime.wasm"
cp targets/browser/host/assets/artifacts/pico-w-signal-pico-local.json "$destination/artifacts/"
cp targets/browser/host/assets/artifacts/pico-w-signal-pico-local.uf2 "$destination/artifacts/"
for target in c3 s3 wroom; do
  test -f "$release_artifacts/esp32-$target-generic-release.bin"
  test -f "$release_artifacts/esp32-$target-generic-release.json"
  cp "$release_artifacts/esp32-$target-generic-release.bin" "$destination/artifacts/"
  cp "$release_artifacts/esp32-$target-generic-release.json" "$destination/artifacts/"
done
cp targets/rp2040/browser-deployment/*.mjs "$destination/targets/rp2040/browser-deployment/"
cp targets/esp32/browser-deployment/*.mjs "$destination/targets/esp32/browser-deployment/"

test "$(find "$destination" -type f | wc -l)" -eq 36
test -z "$(find "$destination" -type f \( -name 'book*.mjs' -o -name 'book*.css' -o -name 'chapter-*.md' \) -print -quit)"
