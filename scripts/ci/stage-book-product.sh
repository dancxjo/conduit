#!/bin/sh
set -eu

runtime=${1:?usage: stage-book-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-book-product.sh RUNTIME DESTINATION}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination"
mkdir -p "$destination/assets"

cp targets/browser/host/assets/book.html "$destination/index.html"
cp targets/browser/host/assets/book.css "$destination/book.css"
cp targets/browser/host/assets/book.mjs "$destination/book.mjs"
cp targets/browser/host/assets/book-state.mjs "$destination/book-state.mjs"
cp targets/browser/host/assets/book-navigation.mjs "$destination/book-navigation.mjs"
cp targets/browser/host/assets/book-runner-presentation.mjs "$destination/book-runner-presentation.mjs"
cp targets/browser/host/assets/book-syntax-editor.mjs "$destination/book-syntax-editor.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp targets/browser/host/assets/browser-host-membership.mjs "$destination/browser-host-membership.mjs"
cp targets/browser/host/assets/browser-application-loader.mjs "$destination/browser-application-loader.mjs"
cp targets/browser/host/assets/browser-application-storage.mjs "$destination/browser-application-storage.mjs"
cp targets/browser/host/assets/application-presentation.mjs "$destination/application-presentation.mjs"
cp "$runtime" "$destination/runtime.wasm"
for asset in react.min.js react-dom.min.js react-flow.min.js react-flow.css flow.css flow.js flow-scene.js flow-layout.js flow-faceplate.js portable-navigation.js; do
    cp "apps/patchbay/html/assets/$asset" "$destination/assets/$asset"
done

chapters='chapter-1.md
chapter-2.md
chapter-3.md
chapter-4.md
chapter-5.md
chapter-6.md
chapter-8.md'
source_chapters=$(find tour/book -maxdepth 1 -type f -name 'chapter-*.md' -printf '%f\n' | LC_ALL=C sort)
test "$source_chapters" = "$chapters"
printf '%s\n' "$chapters" | while IFS= read -r chapter; do
    cp "tour/book/$chapter" "$destination/$chapter"
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
    cp targets/browser/host/assets/book.html "$destination/$route/index.html"
done

node scripts/ci/build-browser-application-package.mjs \
    targets/browser/host/assets/book.application.template.json "$destination"

test "$(find "$destination" -type f | wc -l)" -eq 38
test -z "$(find "$destination" -type f \( -name 'creche*.mjs' -o -name 'creche*.css' -o -path '*/artifacts/*' -o -path '*/targets/*' \) -print -quit)"
