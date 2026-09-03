#!/bin/sh
set -eu

runtime=${1:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}
static_assets=${2:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}
destination=${3:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}

test -f "$runtime"
test -x "$static_assets"
test ! -e "$destination"
mkdir -p "$destination/assets" "$destination/api"
cp apps/patchbay/html/assets/index.html "$destination/index.html"
cp apps/patchbay/html/assets/*.js apps/patchbay/html/assets/*.mjs apps/patchbay/html/assets/*.css "$destination/assets/"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/assets/"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/assets/"
cp targets/browser/host/assets/browser-host-identity.mjs "$destination/assets/"
cp targets/browser/host/assets/application-presentation.mjs "$destination/assets/"
cp targets/browser/host/assets/application-theme.mjs "$destination/assets/"
cp targets/browser/host/assets/application-theme.css "$destination/assets/"
cp targets/browser/host/assets/text-lab-live-runtime.mjs "$destination/assets/"
cp targets/browser/host/assets/websocket-line.mjs "$destination/assets/"
cp "$runtime" "$destination/assets/conduit-browser-runtime.wasm"
"$static_assets" "$destination/api/snapshot" "$destination/assets/theme.css"
node scripts/ci/build-browser-application-package.mjs \
  apps/patchbay/html/assets/patchbay.application.template.json "$destination" patchbay.application.json

test -f "$destination/index.html"
test -f "$destination/patchbay.application.json"
test -f "$destination/api/snapshot"
