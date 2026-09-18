#!/bin/sh
set -eu

usage() {
  echo "usage: install-macos-aarch64.sh [--state-dir PATH]" >&2
}

state_dir="${HOME:?HOME is required}/Library/Application Support/Conduit"
if [ "$#" -gt 0 ]; then
  if [ "$#" -ne 2 ] || [ "$1" != "--state-dir" ] || [ -z "$2" ]; then
    usage
    exit 2
  fi
  state_dir=$2
fi

bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
product=$bundle_dir/conduit-macos-aarch64
manifest=$bundle_dir/hosted-macos-aarch64.json

if [ ! -f "$product" ] || [ ! -f "$manifest" ]; then
  echo "error: the reviewed macOS release bundle is incomplete" >&2
  exit 1
fi

chmod u+x "$product"
exec "$product" host service install "$manifest" --state-dir "$state_dir"
