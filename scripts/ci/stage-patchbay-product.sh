#!/bin/sh
set -eu

host_runtime=${1:?usage: stage-patchbay-product.sh HOST_RUNTIME ENTRANCE_RUNTIME DESTINATION}
entrance_runtime=${2:?usage: stage-patchbay-product.sh HOST_RUNTIME ENTRANCE_RUNTIME DESTINATION}
destination=${3:?usage: stage-patchbay-product.sh HOST_RUNTIME ENTRANCE_RUNTIME DESTINATION}

test -f "$host_runtime"
test -f "$entrance_runtime"
test ! -e "$destination"
mkdir -p "$destination"

cp targets/browser/host/assets/patchbay.html "$destination/index.html"
cp targets/browser/host/assets/patchbay.mjs "$destination/patchbay.mjs"
cp targets/browser/host/assets/patchbay.css "$destination/patchbay.css"
cp targets/browser/host/assets/product-navigation.mjs "$destination/product-navigation.mjs"
cp targets/browser/host/assets/browser-host-membership.mjs "$destination/browser-host-membership.mjs"
cp targets/browser/host/assets/browser-host-identity.mjs "$destination/browser-host-identity.mjs"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/browser-application-loader.mjs"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/browser-application-storage.mjs"
cp targets/browser/host/assets/application-presentation.mjs "$destination/application-presentation.mjs"
cp targets/browser/host/assets/application-theme.mjs "$destination/application-theme.mjs"
cp targets/browser/host/assets/application-theme.css "$destination/application-theme.css"
cp apps/patchbay/html/assets/browser-membership.js "$destination/browser-body-membership.mjs"
cp apps/patchbay/html/assets/body-webrtc-sessions.mjs "$destination/body-webrtc-sessions.mjs"
cp apps/patchbay/html/assets/body-webrtc-session.mjs "$destination/body-webrtc-session.mjs"
cp apps/patchbay/html/assets/webrtc-datachannel-line.mjs "$destination/webrtc-datachannel-line.mjs"
cp apps/patchbay/html/assets/webrtc-session-runtime.mjs "$destination/webrtc-session-runtime.mjs"
cp "$host_runtime" "$destination/runtime.wasm"
cp "$entrance_runtime" "$destination/patchbay-entrance-runtime.wasm"

node scripts/ci/build-browser-application-package.mjs \
    targets/browser/host/assets/patchbay.application.template.json "$destination" patchbay.application.json

test "$(find "$destination" -type f | wc -l)" -eq 19
test -f "$destination/patchbay.application.json"
test -z "$(find "$destination" -type f \( -name 'book*.mjs' -o -name 'creche*.mjs' \) -print -quit)"
