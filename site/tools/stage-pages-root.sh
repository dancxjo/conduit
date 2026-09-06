#!/bin/sh
set -eu

destination=${1:?usage: stage-pages-root.sh DESTINATION}

test ! -e "$destination"
mkdir -p "$destination"
node targets/browser/tools/render-product-masthead.mjs site/index.html "$destination/index.html" home "The Body is the computer."
cp site/site.css "$destination/site.css"
cp targets/browser/host/assets/application-theme.css "$destination/application-theme.css"

test "$(find "$destination" -type f | wc -l)" -eq 3
test -f "$destination/index.html"
test -f "$destination/site.css"
