#!/bin/sh
set -eu

usage() {
  echo "usage: install-linux-x86_64.sh [--state-dir PATH] [--direct-display-tty N --display-configuration FILE --display-image FILE]" >&2
}

state_dir=${XDG_STATE_HOME:-"${HOME:?HOME is required when XDG_STATE_HOME is unset}/.local/state"}/conduit
if [ "${1:-}" = "--state-dir" ]; then
  [ "$#" -ge 2 ] && [ -n "$2" ] || { usage; exit 2; }
  state_dir=$2
  shift 2
fi
bundle_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
product=$bundle_dir/conduit-linux-x86_64
manifest=$bundle_dir/hosted-linux-x86_64.json

if [ ! -f "$product" ] || [ ! -f "$manifest" ]; then
  echo "error: the reviewed Linux release bundle is incomplete" >&2
  exit 1
fi

chmod u+x "$product"
exec "$product" host service install "$manifest" --state-dir "$state_dir" "$@"
