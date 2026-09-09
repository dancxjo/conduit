#!/bin/sh
set -eu
runtime=${1:?usage: stage-workspace-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-workspace-product.sh RUNTIME DESTINATION}
test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination/forms"
cp products/workspace/browser/workspace.html "$destination/index.html"
cp products/workspace/browser/workspace.css products/workspace/browser/workspace.mjs products/workspace/browser/workspace-session.mjs products/workspace/browser/workspace-play.mjs "$destination/"
for asset in creche-lifecycle.mjs creche-form-selection.mjs creche-names.mjs; do
  cp "products/creche/browser/$asset" "$destination/$asset"
done
for asset in browser-audio-cue.mjs browser-body-host.mjs browser-body-input.mjs browser-body-continuity.mjs browser-human-input.mjs browser-form-effects.mjs browser-application-loader.mjs browser-application-storage.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs application-presentation.mjs application-theme.mjs application-syntax-presentation.mjs; do
  cp "targets/browser/host/assets/$asset" "$destination/$asset"
done
cp products/creche/names/catalog.mjs "$destination/creche-name-catalog.mjs"
cp products/shared/browser/conduit.css "$destination/conduit.css"
cp "$runtime" "$destination/runtime.wasm"
cargo xtask forms bundle-initial-body --output "$destination/forms/initial-body.conduit"
node targets/browser/tools/build-browser-application-package.mjs products/workspace/browser/workspace.application.template.json "$destination" workspace.application.json
