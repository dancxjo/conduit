#!/bin/sh
set -eu

destination=${1:?usage: stage-pages-root.sh DESTINATION}
exec cargo xtask make pages-root "$destination"
