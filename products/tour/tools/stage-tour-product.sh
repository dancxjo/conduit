#!/bin/sh
set -eu

runtime=${1:?usage: stage-tour-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-tour-product.sh RUNTIME DESTINATION}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination"
mkdir -p "$destination/assets"

cp products/tour/browser/tour.html "$destination/index.html"
cp products/tour/browser/tour.css "$destination/tour.css"
cp products/tour/browser/tour.mjs "$destination/tour.mjs"
cp products/tour/browser/tour-state.mjs "$destination/tour-state.mjs"
cp targets/browser/host/assets/browser-human-input.mjs "$destination/browser-human-input.mjs"
cp targets/browser/host/assets/browser-form-effects.mjs "$destination/browser-form-effects.mjs"
cp products/tour/browser/tour-navigation.mjs "$destination/tour-navigation.mjs"
cp products/tour/browser/tour-inventory-presentation.mjs "$destination/tour-inventory-presentation.mjs"
cp products/tour/browser/tour-routing.mjs "$destination/tour-routing.mjs"
cp products/tour/browser/tour-runner-presentation.mjs "$destination/tour-runner-presentation.mjs"
cp targets/browser/host/assets/application-syntax-presentation.mjs "$destination/application-syntax-presentation.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp targets/browser/host/assets/browser-host-membership.mjs "$destination/browser-host-membership.mjs"
cp targets/browser/host/assets/browser-host-identity.mjs "$destination/browser-host-identity.mjs"
cp targets/browser/host/assets/browser-host-operations.mjs "$destination/browser-host-operations.mjs"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/browser-application-loader.mjs"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/browser-application-storage.mjs"
cp targets/browser/host/assets/application-presentation.mjs "$destination/application-presentation.mjs"
cp targets/browser/host/assets/application-theme.mjs "$destination/application-theme.mjs"
cp targets/browser/host/assets/application-theme.css "$destination/application-theme.css"
cp semantics/presentation/assets/product-masthead.mjs "$destination/product-masthead.mjs"
cp "$runtime" "$destination/runtime.wasm"
for asset in react.min.js react-dom.min.js react-flow.min.js react-flow.css flow.css flow.js flow-scene.js flow-layout.js flow-faceplate.js portable-navigation.js; do
    cp "products/patchbay/html/assets/$asset" "$destination/assets/$asset"
done

chapters='chapter-1.md
chapter-2.md
chapter-3.md
chapter-4.md
chapter-5.md
chapter-6.md
chapter-8.md'
source_chapters=$(find products/tour/content -maxdepth 1 -type f -name 'chapter-*.md' -printf '%f\n' | LC_ALL=C sort)
test "$source_chapters" = "$chapters"
printf '%s\n' "$chapters" | while IFS= read -r chapter; do
    cp "products/tour/content/$chapter" "$destination/$chapter"
done

page_routes='a-form-you-can-run
faces-backs-and-implementation
hosts-make-forms-real
one-form-across-several-hosts
the-body-one-computer-one-machine-or-many
many-forms-one-body-wide-realization
birth-spores-and-the-creche'
printf '%s\n' "$page_routes" | while IFS= read -r route; do
    mkdir "$destination/$route"
    cp products/tour/browser/tour.html "$destination/$route/index.html"
done

# Preserve the two public chapter URLs published before the chapter-title refresh.
legacy_page_routes='meet-one-gear
same-face-different-implementation'
printf '%s\n' "$legacy_page_routes" | while IFS= read -r route; do
    mkdir "$destination/$route"
    cp products/tour/browser/tour.html "$destination/$route/index.html"
done

node targets/browser/tools/build-browser-application-package.mjs \
    products/tour/browser/tour.application.template.json "$destination" tour.application.json

# Includes the shared admitted Host-effect dispatcher used by Tour and Body.
test -f "$destination/browser-form-effects.mjs"
test "$(find "$destination" -type f | wc -l)" -eq 49
test -z "$(find "$destination" -type f \( -name 'creche*.mjs' -o -name 'creche*.css' -o -path '*/artifacts/*' -o -path '*/targets/*' \) -print -quit)"
