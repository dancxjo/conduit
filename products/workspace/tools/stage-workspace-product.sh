#!/bin/sh
set -eu
runtime=${1:?usage: stage-workspace-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
destination=${2:?usage: stage-workspace-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
release_artifacts=${3:?usage: stage-workspace-product.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
initial_body_bundle=${4:-}
workspace_catalog=${5:-}
test -f "$runtime"
test ! -e "$destination"
test -f "$release_artifacts/release-catalog.json"
mkdir -p "$destination/forms" "$destination/artifacts" "$destination/targets/browser/browser-deployment"
cp products/workspace/browser/workspace.html "$destination/index.html"
cp products/workspace/browser/workspace.webmanifest products/workspace/browser/workspace-icon.svg products/workspace/browser/workspace-share-target-sw.js "$destination/"
cp products/workspace/browser/workspace.css products/workspace/browser/workspace.mjs products/workspace/browser/workspace-session.mjs products/workspace/browser/workspace-membership.mjs products/workspace/browser/workspace-host-configuration.mjs products/workspace/browser/workspace-play.mjs products/workspace/browser/workspace-voice-play.mjs products/workspace/browser/workspace-tutorial-presenter-play.mjs products/workspace/browser/workspace-library.mjs products/workspace/browser/workspace-handoff.mjs products/workspace/browser/workspace-surface.mjs products/workspace/browser/body-bootstrap.mjs products/workspace/browser/reviewed-form-selection.mjs "$destination/"
cp products/creche/browser/creche-browser-configuration.mjs "$destination/browser-host-configuration.mjs"
for asset in creche-names.mjs creche-rendezvous.mjs rendezvous-candidate-schedule.mjs rendezvous-cbor.mjs creche-rendezvous-candidates.mjs creche-physical.mjs creche-physical-presentation.mjs creche-target-catalog.mjs creche-existing-computer.mjs creche-release-catalog.mjs creche-release-bundle.mjs creche-spore-bundle.mjs creche-native-zip.mjs; do
  cp "products/creche/browser/$asset" "$destination/$asset"
done
cp targets/browser/deployment/browser/creche-adapter.mjs targets/browser/deployment/browser/browser-bundle.mjs "$destination/targets/browser/browser-deployment/"
for artifact in browser-page.json runtime.wasm index.html host.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs browser-boot-profile.mjs browser-relay-line.mjs media-host.mjs device-base.mjs usb-device-base.mjs; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
generation=$(node -e 'const fs=require("fs"); const value=JSON.parse(fs.readFileSync(process.argv[1],"utf8")); if(!Number.isSafeInteger(value.generation)||value.generation<1)process.exit(2); process.stdout.write(String(value.generation));' "$release_artifacts/release-catalog.json")
cargo xtask host release-catalog --root "$destination/artifacts" --generation "$generation"
for asset in browser-host-calls.mjs browser-audio-cue.mjs browser-pcm-audio.mjs browser-remote-fragment.mjs browser-remote-voice.mjs browser-body-host.mjs browser-body-input.mjs browser-body-continuity.mjs browser-human-input.mjs browser-form-effects.mjs browser-application-loader.mjs browser-application-storage.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs application-presentation.mjs application-theme.mjs application-syntax-presentation.mjs; do
  cp "targets/browser/host/assets/$asset" "$destination/$asset"
done
cp products/creche/names/catalog.mjs "$destination/creche-name-catalog.mjs"
cp products/shared/browser/conduit.css "$destination/conduit.css"
cp "$runtime" "$destination/runtime.wasm"
if test -n "$initial_body_bundle" || test -n "$workspace_catalog"; then
  test -n "$initial_body_bundle"
  test -n "$workspace_catalog"
  test -f "$initial_body_bundle"
  test -f "$workspace_catalog"
  cp "$initial_body_bundle" "$destination/forms/initial-body.conduit"
  cp "$workspace_catalog" "$destination/forms/workspace-catalog.json"
else
  cargo xtask forms bundle-initial-body --output "$destination/forms/initial-body.conduit"
  cargo xtask forms bundle-workspace-catalog --output "$destination/forms/workspace-catalog.json"
fi
node targets/browser/tools/build-browser-application-package.mjs products/workspace/browser/workspace.application.template.json "$destination" workspace.application.json
