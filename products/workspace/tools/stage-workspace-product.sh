#!/bin/sh
set -eu
runtime=${1:?usage: stage-workspace-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-workspace-product.sh RUNTIME DESTINATION}
test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination/forms"
cp products/workspace/browser/workspace.html "$destination/index.html"
cp products/workspace/browser/workspace.css products/workspace/browser/resident-tour.css products/workspace/browser/workspace.mjs products/workspace/browser/body-tutorial.mjs products/workspace/browser/workspace-session.mjs products/workspace/browser/workspace-membership.mjs products/workspace/browser/workspace-host-configuration.mjs products/workspace/browser/workspace-play.mjs products/workspace/browser/workspace-voice-play.mjs products/workspace/browser/workspace-library.mjs products/workspace/browser/workspace-handoff.mjs products/workspace/browser/workspace-surface.mjs products/workspace/browser/body-bootstrap.mjs products/workspace/browser/reviewed-form-selection.mjs "$destination/"
cp products/creche/browser/creche-browser-configuration.mjs "$destination/browser-host-configuration.mjs"
for asset in creche-names.mjs creche-rendezvous.mjs; do
  cp "products/creche/browser/$asset" "$destination/$asset"
done
for asset in browser-host-operations.mjs browser-audio-cue.mjs browser-pcm-audio.mjs browser-remote-fragment.mjs browser-remote-voice.mjs browser-body-host.mjs browser-body-input.mjs browser-body-continuity.mjs browser-human-input.mjs browser-form-effects.mjs browser-application-loader.mjs browser-application-storage.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs application-presentation.mjs application-theme.mjs application-syntax-presentation.mjs; do
  cp "targets/browser/host/assets/$asset" "$destination/$asset"
done
cp products/creche/names/catalog.mjs "$destination/creche-name-catalog.mjs"
cp products/shared/browser/conduit.css "$destination/conduit.css"
cp "$runtime" "$destination/runtime.wasm"
cargo xtask forms bundle-initial-body --output "$destination/forms/initial-body.conduit"
cargo xtask forms bundle-workspace-catalog --output "$destination/forms/workspace-catalog.json"
node targets/browser/tools/build-browser-application-package.mjs products/workspace/browser/workspace.application.template.json "$destination" workspace.application.json
