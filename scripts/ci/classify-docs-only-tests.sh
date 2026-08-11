#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
classifier="$repo_root/scripts/ci/classify-docs-only.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

git -C "$fixture" init -q
git -C "$fixture" config user.name "Conduit CI fixture"
git -C "$fixture" config user.email "ci-fixture@invalid.example"

commit_file() {
  local path="$1" contents="$2"
  mkdir -p "$fixture/$(dirname "$path")"
  printf '%s\n' "$contents" > "$fixture/$path"
  git -C "$fixture" add "$path"
  git -C "$fixture" commit -qm "fixture $path"
  git -C "$fixture" rev-parse HEAD
}

assert_classification() {
  local expected="$1" base="$2" head="$3" output
  output="$(cd "$fixture" && "$classifier" "$base" "$head")"
  grep -qx "docs_only=$expected" <<<"$output"
}

base="$(commit_file README.md base)"
docs="$(commit_file docs/guide.md docs)"
assert_classification true "$base" "$docs"

source_change="$(commit_file crates/example/src/lib.rs code)"
assert_classification false "$docs" "$source_change"

manifest_change="$(commit_file crates/example/Cargo.toml manifest)"
assert_classification false "$source_change" "$manifest_change"

lock_change="$(commit_file Cargo.lock lock)"
assert_classification false "$manifest_change" "$lock_change"

workflow_change="$(commit_file .github/workflows/check.yml workflow)"
assert_classification false "$lock_change" "$workflow_change"

baseline_change="$(commit_file hosts/browser/baseline.png pixels)"
assert_classification false "$workflow_change" "$baseline_change"

mixed_base="$baseline_change"
printf '%s\n' mixed >> "$fixture/README.md"
printf '%s\n' mixed >> "$fixture/crates/example/src/lib.rs"
git -C "$fixture" add README.md crates/example/src/lib.rs
git -C "$fixture" commit -qm "mixed fixture"
mixed="$(git -C "$fixture" rev-parse HEAD)"
assert_classification false "$mixed_base" "$mixed"

assert_classification false 0000000000000000000000000000000000000000 "$mixed"
assert_classification false "$mixed" "$mixed"
printf 'docs-only classifier fixtures passed\n'
