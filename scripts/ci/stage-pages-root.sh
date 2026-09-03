#!/bin/sh
set -eu

destination=${1:?usage: stage-pages-root.sh DESTINATION}

test ! -e "$destination"
mkdir -p "$destination"
cp site/index.html "$destination/index.html"
cp site/site.css "$destination/site.css"
cp targets/browser/host/assets/product-navigation.mjs "$destination/product-navigation.mjs"

test "$(find "$destination" -type f | wc -l)" -eq 3
test -f "$destination/index.html"
test -f "$destination/site.css"
test -f "$destination/product-navigation.mjs"
