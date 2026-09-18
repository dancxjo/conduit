#!/bin/sh
set -eu

runtime=${1:?usage: stage-home-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-home-product.sh RUNTIME DESTINATION}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination"
cp products/home/browser/home.html "$destination/index.html"
cp products/home/browser/home.css "$destination/home.css"
cp products/home/browser/home.mjs "$destination/home.mjs"
cp forms/hello/main.conduit "$destination/hello.conduit"
cp targets/browser/host/assets/application-presentation.mjs "$destination/application-presentation.mjs"
cp targets/browser/host/assets/application-theme.mjs "$destination/application-theme.mjs"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/browser-application-loader.mjs"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/browser-application-storage.mjs"
cp "$runtime" "$destination/runtime.wasm"
node targets/browser/tools/build-browser-application-package.mjs \
  products/home/browser/home.application.template.json "$destination" home.application.json

test "$(find "$destination" -type f | wc -l)" -eq 10
