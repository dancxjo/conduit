#!/usr/bin/env bash
set -euo pipefail

base_sha="${1:-}"
head_sha="${2:-}"
comparison_base_sha=""

full() {
  printf 'docs_only=false\nreason=%s\nbase_sha=%s\nhead_sha=%s\ncomparison_base_sha=%s\n' "$1" "$base_sha" "$head_sha" "$comparison_base_sha"
  exit 0
}

[[ "$base_sha" =~ ^[0-9a-fA-F]{40}$ ]] || full "missing-or-invalid-base"
[[ "$head_sha" =~ ^[0-9a-fA-F]{40}$ ]] || full "missing-or-invalid-head"
[[ "$base_sha" != 0000000000000000000000000000000000000000 ]] || full "missing-or-invalid-base"
git cat-file -e "${base_sha}^{commit}" 2>/dev/null || full "unavailable-base"
git cat-file -e "${head_sha}^{commit}" 2>/dev/null || full "unavailable-head"

mapfile -t merge_bases < <(git merge-base --all "$base_sha" "$head_sha")
((${#merge_bases[@]} == 1)) || full "missing-or-ambiguous-merge-base"
comparison_base_sha="${merge_bases[0]}"

# Exhaustive promotion needs the controller and proof plans even when the
# frozen snapshot changes only documentation. Publish one classification for
# both this job's prerequisites and the downstream proof jobs.
[[ "${CONDUIT_FULL_SUITE:-false}" != true ]] || full "full-suite"

paths=()
while IFS= read -r -d '' path; do
  paths+=("$path")
done < <(git diff --name-only -z --diff-filter=ACDMRTUXB "$comparison_base_sha" "$head_sha")

((${#paths[@]} > 0)) || full "empty-change-set"
for path in "${paths[@]}"; do
  [[ "$path" == *.md ]] || full "non-markdown-change"
  # A newly tracked source-owner path needs the structural ownership guard,
  # even when its only content is Markdown. Existing documentation stays cheap.
  if [[ "$path" != docs/* ]] && ! git cat-file -e "$comparison_base_sha:$path" 2>/dev/null; then
    full "new-source-owner-markdown"
  fi
done

if git cat-file -e "$head_sha:docs/visual-evidence.md" 2>/dev/null; then
  visual_docs="$(git show "$head_sha:docs/visual-evidence.md")"
  for scenario in selected-gear interaction disconnected; do
    grep -Fq "current/patchbay/$scenario.png)](https://dancxjo.github.io/conduit/current/patchbay/$scenario/)" <<<"$visual_docs" \
      || full "canonical-visual-reference-drift"
  done
  if git grep -q 'https://dancxjo.github.io/conduit/commits/' "$head_sha" -- '*.md'; then
    full "immutable-visual-reference"
  fi
fi

printf 'docs_only=true\nreason=all-markdown\nbase_sha=%s\nhead_sha=%s\ncomparison_base_sha=%s\n' "$base_sha" "$head_sha" "$comparison_base_sha"
