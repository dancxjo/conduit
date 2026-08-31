#!/bin/sh
set -eu

runtime=${1:?usage: stage-book-product.sh RUNTIME DESTINATION}
destination=${2:?usage: stage-book-product.sh RUNTIME DESTINATION}

test -f "$runtime"
test ! -e "$destination"
mkdir -p "$destination"

cp targets/browser/host/assets/book.html "$destination/index.html"
cp targets/browser/host/assets/book.css "$destination/book.css"
cp targets/browser/host/assets/book.mjs "$destination/book.mjs"
cp targets/browser/host/assets/browser-host-bootstrap.mjs "$destination/browser-host-bootstrap.mjs"
cp "$runtime" "$destination/runtime.wasm"

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

test "$(find "$destination" -type f | wc -l)" -eq 12
test -z "$(find "$destination" -type f \( -name 'creche*.mjs' -o -name 'creche*.css' -o -path '*/artifacts/*' -o -path '*/targets/*' \) -print -quit)"
