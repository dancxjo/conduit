#!/bin/sh
set -eu

runtime=${1:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
destination=${2:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}
release_artifacts=${3:?usage: stage-creche-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination/artifacts" "$destination/forms" "$destination/targets/avr/browser-deployment" "$destination/targets/rp2040/browser-deployment" "$destination/targets/esp32/browser-deployment" "$destination/targets/std/browser-deployment" "$destination/targets/browser/browser-deployment" "$destination/targets/orange-pi/browser-deployment" "$destination/targets/raspberry-pi/browser-deployment" "$destination/targets/conduitos/browser-deployment"

cp products/creche/browser/creche.html "$destination/index.html"
cp products/creche/browser/creche.css "$destination/creche.css"
cp products/creche/browser/creche.mjs "$destination/creche.mjs"
cargo xtask forms bundle-initial-body --output "$destination/forms/initial-body.conduit"
cp products/creche/browser/creche-lifecycle.mjs "$destination/creche-lifecycle.mjs"
cp products/creche/browser/creche-form-selection.mjs "$destination/creche-form-selection.mjs"
cp products/creche/browser/creche-names.mjs "$destination/creche-names.mjs"
cp products/creche/browser/creche-physical.mjs "$destination/creche-physical.mjs"
cp products/creche/browser/creche-physical-presentation.mjs "$destination/creche-physical-presentation.mjs"
cp products/creche/browser/creche-target-catalog.mjs "$destination/creche-target-catalog.mjs"
cp products/creche/browser/creche-browser-configuration.mjs "$destination/creche-browser-configuration.mjs"
cp products/creche/browser/creche-spore-bundle.mjs "$destination/creche-spore-bundle.mjs"
cp products/creche/browser/creche-native-zip.mjs "$destination/creche-native-zip.mjs"
cp products/creche/browser/creche-native-disk.mjs "$destination/creche-native-disk.mjs"
cp products/creche/browser/creche-release-bundle.mjs "$destination/creche-release-bundle.mjs"
cp products/creche/browser/creche-existing-computer.mjs "$destination/creche-existing-computer.mjs"
cp products/creche/browser/creche-graduation.mjs "$destination/creche-graduation.mjs"
cp products/creche/browser/creche-routing.mjs "$destination/creche-routing.mjs"
cp targets/browser/host/assets/application-syntax-presentation.mjs "$destination/application-syntax-presentation.mjs"
cp targets/browser/host/assets/application-presentation.mjs "$destination/application-presentation.mjs"
cp targets/browser/host/assets/application-theme.mjs "$destination/application-theme.mjs"
cp targets/browser/host/assets/application-theme.css "$destination/application-theme.css"
cp semantics/presentation/assets/product-masthead.mjs "$destination/product-masthead.mjs"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/browser-application-loader.mjs"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/browser-application-storage.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp targets/browser/host/assets/browser-host-membership.mjs "$destination/browser-host-membership.mjs"
cp targets/browser/host/assets/browser-host-identity.mjs "$destination/browser-host-identity.mjs"
cp targets/browser/host/assets/browser-host-operations.mjs "$destination/browser-host-operations.mjs"
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
for artifact in hosted-linux-x86_64.json conduit-linux-x86_64 hosted-windows-x86_64.json conduit-windows-x86_64.exe hosted-macos-aarch64.json conduit-macos-aarch64 browser-page.json runtime.wasm index.html host.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs browser-boot-profile.mjs media-host.mjs device-base.mjs usb-device-base.mjs; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in avr-promicro-atmega32u4-5v-16mhz.json promicro-atmega32u4-5v-16mhz.hex; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in orange-pi-5-image.json conduitos-orange-pi-5.img raspios-bookworm-pi4-model-b-rev-1.5-4gb.json raspios-bookworm-zero-2-w-rev-1.0.json raspios-bookworm-zero-2-wh-rev-1.0.json conduit-linux-aarch64 rpi-b-plus-image.json conduitos-rpi-b-plus.img rpi-zero-v1-image.json conduitos-rpi-zero-v1.img rpi-zero-w-v1.1-image.json conduitos-rpi-zero-w-v1.1.img rpi-zero-wh-v1.1-image.json conduitos-rpi-zero-wh-v1.1.img; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in conduitos-x86_64-pc-release.json conduitos-x86_64-pc.iso conduitos-aarch64-virt-release.json conduitos-aarch64-virt.iso conduitos-ia32-pc-release.json conduitos-ia32-pc.iso conduitos-riscv64-virt-release.json conduitos-riscv64-virt.iso conduitos-loongarch64-virt-release.json conduitos-loongarch64-virt.iso; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
cp targets/avr/deployment/browser/*.mjs "$destination/targets/avr/browser-deployment/"
cp targets/rp2040/deployment/browser/*.mjs "$destination/targets/rp2040/browser-deployment/"
cp targets/esp32/deployment/browser/*.mjs "$destination/targets/esp32/browser-deployment/"
cp targets/std/deployment/browser/*.mjs "$destination/targets/std/browser-deployment/"
cp targets/browser/deployment/browser/*.mjs "$destination/targets/browser/browser-deployment/"
cp targets/orange-pi/deployment/browser/*.mjs "$destination/targets/orange-pi/browser-deployment/"
cp targets/raspberry-pi/deployment/browser/*.mjs "$destination/targets/raspberry-pi/browser-deployment/"
cp targets/conduitos/deployment/browser/*.mjs "$destination/targets/conduitos/browser-deployment/"

for route in birth first-host physical-host graduate; do
  mkdir "$destination/$route"
  cp products/creche/browser/creche.html "$destination/$route/index.html"
done

node targets/browser/tools/build-browser-application-package.mjs \
  products/creche/browser/creche.application.template.json "$destination" creche.application.json

file_count=$(find "$destination" -type f | wc -l)
test "$file_count" -le 128
test -f "$destination/creche.application.json"
test -f "$destination/creche-browser-configuration.mjs"
test -z "$(find "$destination" -type f \( -name 'book*.mjs' -o -name 'book*.css' -o -name 'chapter-*.md' \) -print -quit)"
