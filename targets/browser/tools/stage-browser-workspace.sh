#!/bin/sh
set -eu
runtime=${1:?usage: stage-browser-workspace.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
destination=${2:?usage: stage-browser-workspace.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
release_artifacts=${3:?usage: stage-browser-workspace.sh RUNTIME DESTINATION RELEASE_ARTIFACTS [INITIAL_BODY_BUNDLE WORKSPACE_CATALOG]}
initial_body_bundle=${4:-}
workspace_catalog=${5:-}
test -f "$runtime"
test ! -e "$destination"
test -f "$release_artifacts/release-catalog.json"
mkdir -p "$destination/plots" "$destination/artifacts" "$destination/targets/avr/browser-deployment" "$destination/targets/rp2040/browser-deployment" "$destination/targets/esp32/browser-deployment" "$destination/targets/std/browser-deployment" "$destination/targets/browser/browser-deployment" "$destination/targets/orange-pi/browser-deployment" "$destination/targets/raspberry-pi/browser-deployment" "$destination/targets/conduitos/browser-deployment"
cp targets/browser/workspace/workspace.html "$destination/index.html"
node --input-type=module - "$destination/index.html" <<'JAVASCRIPT'
import { readFileSync, writeFileSync } from 'node:fs';
const file = process.argv[2];
const styles = readFileSync('site/chrome.css', 'utf8');
const navigation = readFileSync('site/navigation.html', 'utf8');
const html = readFileSync(file, 'utf8');
writeFileSync(file, html.replace('</head>', `<style>${styles}</style></head>`)
  .replace(/(<body\b[^>]*>)/, `$1${navigation}`));
JAVASCRIPT
cp targets/browser/workspace/workspace.webmanifest targets/browser/workspace/workspace-icon.svg targets/browser/workspace/workspace-share-target-sw.js "$destination/"
cp targets/browser/workspace/workspace.css targets/browser/workspace/workspace.mjs targets/browser/workspace/workspace-session.mjs targets/browser/workspace/workspace-membership.mjs targets/browser/workspace/workspace-host-configuration.mjs targets/browser/workspace/workspace-play.mjs targets/browser/workspace/workspace-voice-play.mjs targets/browser/workspace/workspace-tutorial-presenter-play.mjs targets/browser/workspace/workspace-library.mjs targets/browser/workspace/workspace-handoff.mjs targets/browser/workspace/workspace-surface.mjs targets/browser/workspace/body-bootstrap.mjs targets/browser/workspace/reviewed-plot-selection.mjs "$destination/"
cp targets/browser/workspace/browser-host-configuration.mjs "$destination/browser-host-configuration.mjs"
for asset in creche-rendezvous.mjs rendezvous-candidate-schedule.mjs rendezvous-cbor.mjs creche-rendezvous-candidates.mjs creche-physical.mjs creche-physical-presentation.mjs creche-target-catalog.mjs creche-installed-targets.mjs creche-existing-computer.mjs creche-release-catalog.mjs creche-release-bundle.mjs creche-spore-bundle.mjs creche-native-zip.mjs creche-native-disk.mjs; do
  cp "targets/browser/workspace/$asset" "$destination/$asset"
done
cp targets/browser/deployment/browser/creche-adapter.mjs targets/browser/deployment/browser/browser-bundle.mjs "$destination/targets/browser/browser-deployment/"
cp targets/avr/deployment/browser/*.mjs "$destination/targets/avr/browser-deployment/"
cp targets/rp2040/deployment/browser/*.mjs "$destination/targets/rp2040/browser-deployment/"
cp targets/esp32/deployment/browser/*.mjs "$destination/targets/esp32/browser-deployment/"
cp targets/std/deployment/browser/*.mjs "$destination/targets/std/browser-deployment/"
cp targets/orange-pi/deployment/browser/*.mjs "$destination/targets/orange-pi/browser-deployment/"
cp targets/raspberry-pi/deployment/browser/*.mjs "$destination/targets/raspberry-pi/browser-deployment/"
cp targets/conduitos/deployment/browser/*.mjs "$destination/targets/conduitos/browser-deployment/"
for artifact in browser-page.json runtime.wasm index.html host.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs browser-boot-profile.mjs browser-relay-line.mjs media-host.mjs device-base.mjs usb-device-base.mjs; do
  test -f "$release_artifacts/$artifact"
  cp "$release_artifacts/$artifact" "$destination/artifacts/"
done
for artifact in "$release_artifacts"/*; do
  test "$(basename "$artifact")" = release-catalog.json && continue
  test -f "$artifact" || continue
  cp "$artifact" "$destination/artifacts/"
done
generation=$(node -e 'const fs=require("fs"); const value=JSON.parse(fs.readFileSync(process.argv[1],"utf8")); if(!Number.isSafeInteger(value.generation)||value.generation<1)process.exit(2); process.stdout.write(String(value.generation));' "$release_artifacts/release-catalog.json")
cargo xtask make host release-catalog --root "$destination/artifacts" --generation "$generation"
for asset in browser-host-calls.mjs browser-audio-cue.mjs browser-pcm-audio.mjs browser-remote-fragment.mjs browser-remote-voice.mjs browser-body-host.mjs browser-monotonic-timer.mjs browser-body-input.mjs browser-body-continuity.mjs browser-human-input.mjs browser-plot-effects.mjs browser-runtime-bridge.mjs browser-application-loader.mjs browser-application-storage.mjs browser-host-bootstrap.mjs browser-host-membership.mjs browser-host-identity.mjs application-presentation.mjs application-graph-canvas.mjs application-theme.mjs application-syntax-presentation.mjs device-base.mjs usb-device-base.mjs; do
  cp "targets/browser/host/assets/$asset" "$destination/$asset"
done
cp targets/browser/host/assets/conduit.css "$destination/conduit.css"
cp "$runtime" "$destination/runtime.wasm"
if test -n "$initial_body_bundle" || test -n "$workspace_catalog"; then
  test -n "$initial_body_bundle"
  test -n "$workspace_catalog"
  test -f "$initial_body_bundle"
  test -f "$workspace_catalog"
  cp "$initial_body_bundle" "$destination/plots/initial-body.conduit"
  cp "$workspace_catalog" "$destination/plots/workspace-catalog.json"
else
  cargo xtask check plots bundle-initial-body --output "$destination/plots/initial-body.conduit"
  cargo xtask check plots bundle-workspace-catalog --output "$destination/plots/workspace-catalog.json"
fi
node targets/browser/tools/build-browser-application-package.mjs targets/browser/workspace/workspace.application.template.json "$destination" workspace.application.json
