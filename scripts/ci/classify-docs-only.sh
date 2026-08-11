#!/usr/bin/env bash
set -euo pipefail

base_sha="${1:-}"
head_sha="${2:-}"

full() {
  printf 'docs_only=false\nreason=%s\nbase_sha=%s\nhead_sha=%s\n' "$1" "$base_sha" "$head_sha"
  exit 0
}

[[ "$base_sha" =~ ^[0-9a-fA-F]{40}$ ]] || full "missing-or-invalid-base"
[[ "$head_sha" =~ ^[0-9a-fA-F]{40}$ ]] || full "missing-or-invalid-head"
[[ "$base_sha" != 0000000000000000000000000000000000000000 ]] || full "missing-or-invalid-base"
git cat-file -e "${base_sha}^{commit}" 2>/dev/null || full "unavailable-base"
git cat-file -e "${head_sha}^{commit}" 2>/dev/null || full "unavailable-head"

paths=()
while IFS= read -r -d '' path; do
  paths+=("$path")
done < <(git diff --name-only -z --diff-filter=ACDMRTUXB "$base_sha" "$head_sha")

((${#paths[@]} > 0)) || full "empty-change-set"
for path in "${paths[@]}"; do
  [[ "$path" == *.md ]] || full "non-markdown-change"
done

if git cat-file -e "$head_sha:README.md" 2>/dev/null \
  && git cat-file -e "$head_sha:docs/visual-evidence.md" 2>/dev/null; then
  readme="$(git show "$head_sha:README.md")"
  visual_docs="$(git show "$head_sha:docs/visual-evidence.md")"
  grep -Fq 'current/patchbay/overview.png)](https://dancxjo.github.io/conduit/current/patchbay/overview/)' <<<"$readme" \
    || full "canonical-visual-reference-drift"
  for scenario in selected-gear interaction disconnected; do
    grep -Fq "current/patchbay/$scenario.png)](https://dancxjo.github.io/conduit/current/patchbay/$scenario/)" <<<"$visual_docs" \
      || full "canonical-visual-reference-drift"
  done
  if git grep -q 'https://dancxjo.github.io/conduit/commits/' "$head_sha" -- '*.md'; then
    full "immutable-visual-reference"
  fi
fi

printf 'docs_only=true\nreason=all-markdown\nbase_sha=%s\nhead_sha=%s\n' "$base_sha" "$head_sha"
