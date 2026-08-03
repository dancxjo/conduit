#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
dependency_dir=${1:-"$script_dir/.deps"}
mkdir -p "$dependency_dir"

fetch() {
  local url=$1
  local expected=$2
  local output=$3
  if [[ ! -f "$output" ]]; then
    curl --fail --location --silent --show-error --output "$output" "$url"
  fi
  printf '%s  %s\n' "$expected" "$output" | sha256sum --check --status
}

fetch \
  "https://repo1.maven.org/maven2/io/projectreactor/reactor-core/3.8.6/reactor-core-3.8.6.jar" \
  "ffc646b225465efce55d3ea350f1bcd1d27d4a56e912323e316dd8e6d04bff11" \
  "$dependency_dir/reactor-core-3.8.6.jar"
fetch \
  "https://repo1.maven.org/maven2/org/reactivestreams/reactive-streams/1.0.4/reactive-streams-1.0.4.jar" \
  "f75ca597789b3dac58f61857b9ac2e1034a68fa672db35055a8fb4509e325f28" \
  "$dependency_dir/reactive-streams-1.0.4.jar"
