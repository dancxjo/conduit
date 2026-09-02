#!/bin/sh
set -eu

runtime=${1:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
destination=${2:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
release_artifacts=${3:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination/artifacts" "$destination/targets/avr/browser-deployment" "$destination/targets/rp2040/browser-deployment" "$destination/targets/esp32/browser-deployment" "$destination/targets/std/browser-deployment" "$destination/targets/browser/browser-deployment" "$destination/targets/orange-pi/browser-deployment" "$destination/targets/raspberry-pi/browser-deployment" "$destination/targets/conduitos/browser-deployment"

cp targets/browser/host/assets/creche.html "$destination/index.html"
cp targets/browser/host/assets/creche.css "$destination/creche.css"
cp targets/browser/host/assets/creche.mjs "$destination/creche.mjs"
cp targets/browser/host/assets/creche-lifecycle.mjs "$destination/creche-lifecycle.mjs"
cp targets/browser/host/assets/creche-physical.mjs "$destination/creche-physical.mjs"
cp targets/browser/host/assets/creche-target-catalog.mjs "$destination/creche-target-catalog.mjs"
cp targets/browser/host/assets/creche-spore-bundle.mjs "$destination/creche-spore-bundle.mjs"
cp targets/browser/host/assets/creche-native-zip.mjs "$destination/creche-native-zip.mjs"
cp targets/browser/host/assets/creche-native-disk.mjs "$destination/creche-native-disk.mjs"
cp targets/browser/host/assets/creche-release-bundle.mjs "$destination/creche-release-bundle.mjs"
cp targets/browser/host/assets/creche-existing-computer.mjs "$destination/creche-existing-computer.mjs"
cp targets/browser/host/assets/creche-graduation.mjs "$destination/creche-graduation.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp targets/browser/host/assets/browser-host-membership.mjs "$destination/browser-host-membership.mjs"
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
for artifact in hosted-linux-x86_64.json conduit-linux-x86_64 browser-page.json runtime.wasm index.html host.mjs browser-host-bootstrap.mjs browser-host-membership.mjs media-host.mjs device-base.mjs usb-device-base.mjs; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in avr-promicro-atmega32u4-5v-16mhz.json promicro-atmega32u4-5v-16mhz.hex; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in orange-pi-5-image.json conduitos-orange-pi-5.img raspios-bookworm-pi4-model-b-rev-1.5-4gb.json conduit-linux-aarch64 rpi-b-plus-image.json conduitos-rpi-b-plus.img; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in conduitos-x86_64-pc-release.json conduitos-x86_64-pc.iso conduitos-aarch64-virt-release.json conduitos-aarch64-virt.iso; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
cp targets/avr/browser-deployment/*.mjs "$destination/targets/avr/browser-deployment/"
cp targets/rp2040/browser-deployment/*.mjs "$destination/targets/rp2040/browser-deployment/"
cp targets/esp32/browser-deployment/*.mjs "$destination/targets/esp32/browser-deployment/"
cp targets/std/browser-deployment/*.mjs "$destination/targets/std/browser-deployment/"
cp targets/browser/host/browser-deployment/*.mjs "$destination/targets/browser/browser-deployment/"
cp targets/orange-pi/browser-deployment/*.mjs "$destination/targets/orange-pi/browser-deployment/"
cp targets/raspberry-pi/browser-deployment/*.mjs "$destination/targets/raspberry-pi/browser-deployment/"
cp targets/conduitos/browser-deployment/*.mjs "$destination/targets/conduitos/browser-deployment/"

for route in birth first-host physical-host graduate; do
  mkdir "$destination/$route"
  cp targets/browser/host/assets/creche.html "$destination/$route/index.html"
done

test "$(find "$destination" -type f | wc -l)" -eq 74
test -z "$(find "$destination" -type f \( -name 'book*.mjs' -o -name 'book*.css' -o -name 'chapter-*.md' \) -print -quit)"
