#!/bin/sh
set -eu

root=${1:-target}
manifest="$root/staged-workspace-artifacts.txt"
source_root="$root/creche-product/artifacts"
destination_root="$root/workspace-product/artifacts"

test -f "$manifest"
test -d "$source_root"
test ! -e "$destination_root" || test -z "$(find "$destination_root" -mindepth 1 -print -quit)"
mkdir -p "$destination_root"

while IFS= read -r relative; do
  case "$relative" in
    ./*) ;;
    *) echo "invalid staged Workspace artifact path: $relative" >&2; exit 2 ;;
  esac
  case "$relative" in
    *../*|*/..|../*) echo "escaping staged Workspace artifact path: $relative" >&2; exit 2 ;;
  esac
  source="$source_root/${relative#./}"
  destination="$destination_root/${relative#./}"
  test -f "$source"
  mkdir -p "$(dirname "$destination")"
  ln "$source" "$destination"
done < "$manifest"

test "$(find "$destination_root" -type f | wc -l)" -eq "$(wc -l < "$manifest")"
