#!/bin/sh
set -eu

runtime=${1:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}
static_assets=${2:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}
destination=${3:?usage: stage-patchbay-product.sh RUNTIME STATIC_ASSETS DESTINATION}

test -f "$runtime"
test -x "$static_assets"
test ! -e "$destination"
mkdir -p "$destination/assets" "$destination/api"
cp products/patchbay/html/assets/index.html "$destination/index.html"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/assets/"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/assets/"
"$static_assets" "$destination/api/snapshot" "$destination/assets/theme.css" "$runtime" "$destination"

test -f "$destination/index.html"
test -f "$destination/patchbay.application.json"
test -f "$destination/api/snapshot"
