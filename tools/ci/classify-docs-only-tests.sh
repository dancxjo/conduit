#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
classifier="$repo_root/tools/ci/classify-docs-only.sh"
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

canonical_visual='current/patchbay/selected-gear.png)](https://dancxjo.github.io/conduit/current/patchbay/selected-gear/)
current/patchbay/interaction.png)](https://dancxjo.github.io/conduit/current/patchbay/interaction/)
current/patchbay/disconnected.png)](https://dancxjo.github.io/conduit/current/patchbay/disconnected/)'
visual_docs="$(commit_file docs/visual-evidence.md "$canonical_visual")"
assert_classification true "$docs" "$visual_docs"

readme_rewrite="$(commit_file README.md 'body-building README without a Patchbay image')"
assert_classification true "$visual_docs" "$readme_rewrite"

broken_visual="$(commit_file docs/visual-evidence.md 'missing canonical visual links')"
assert_classification false "$readme_rewrite" "$broken_visual"

restored_visual="$(commit_file docs/visual-evidence.md "$canonical_visual")"
assert_classification true "$broken_visual" "$restored_visual"

source_change="$(commit_file architecture/example/src/lib.rs code)"
assert_classification false "$restored_visual" "$source_change"

manifest_change="$(commit_file architecture/example/Cargo.toml manifest)"
assert_classification false "$source_change" "$manifest_change"

lock_change="$(commit_file Cargo.lock lock)"
assert_classification false "$manifest_change" "$lock_change"

workflow_change="$(commit_file .github/workflows/check.yml workflow)"
assert_classification false "$lock_change" "$workflow_change"

baseline_change="$(commit_file proof/browser/baseline.png pixels)"
assert_classification false "$workflow_change" "$baseline_change"

mixed_base="$baseline_change"
printf '%s\n' mixed >> "$fixture/README.md"
printf '%s\n' mixed >> "$fixture/architecture/example/src/lib.rs"
git -C "$fixture" add README.md architecture/example/src/lib.rs
git -C "$fixture" commit -qm "mixed fixture"
mixed="$(git -C "$fixture" rev-parse HEAD)"
assert_classification false "$mixed_base" "$mixed"

# Advancing main through an unrelated source change must not contaminate an
# unchanged documentation candidate's own change set.
git -C "$fixture" checkout -qb candidate-docs "$mixed"
candidate_docs="$(commit_file docs/candidate.md candidate)"
git -C "$fixture" checkout -qb advanced-main "$mixed"
advanced_main="$(commit_file targets/esp32/unrelated.rs unrelated)"
classification="$(cd "$fixture" && "$classifier" "$advanced_main" "$candidate_docs")"
grep -qx 'docs_only=true' <<<"$classification"
grep -qx "comparison_base_sha=$mixed" <<<"$classification"

assert_classification false 0000000000000000000000000000000000000000 "$mixed"
assert_classification false "$mixed" "$mixed"
printf 'docs-only classifier fixtures passed\n'
