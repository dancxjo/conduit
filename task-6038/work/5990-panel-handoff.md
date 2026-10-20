Stage05 panel preparation handoff (uncommitted, no QEMU/browser/CI acceptance)

The renderer now uses checked recipe region order for wide visual geometry while restoring the exact original Source row/control vector. The first authored group pair occupies adjacent lanes; later groups follow below it. Unmatched rows before the first matched Source row form a complete prefix; remaining unmatched rows form a complete tail. Narrow/generic/Details retain the existing linear path. Face and relationships are unchanged. Pages may be nonmonotonic in Source order; pages() uses max and focus/hits resolve exact row pages.

Validation: component_recipe 7 passed; native_workset 16 passed; native_face_scene 84 passed; front_door::arrival 10 passed; package Clippy --lib --tests --features native-compositor with warnings denied passed. Final source differs from scene-complete only by an accurate permutation comment and integration harness import (public front_door), both included in final Clippy. All red/green logs retained in work/5990-panel-logs. Resource/file/log hashes and explicit bounds are in work/5990-panel-resource-receipt.json.

Owned changed paths:
- targets/conduitos/src/native_face_scene.rs
- targets/conduitos/src/native_face_scene/document.rs
- targets/conduitos/src/native_face_scene/layout.rs
- targets/conduitos/src/native_face_scene/layout/panels.rs
- targets/conduitos/src/native_face_scene/layout/panels/ordering.rs
- targets/conduitos/src/native_face_scene/layout/panels/tests.rs
- targets/conduitos/src/native_face_scene/layout/panels/tests/creche.rs
- targets/conduitos/tests/native_face_scene.rs (harness import only)

Resources: Row 88→96 bytes, +262,144 bytes at 32,768 rows. Three-word row permutation scratch ≤786,432 bytes. Conservative simultaneous indexed Item + Row + permutation element capacities ≤7,077,888 bytes; existing text/Face/Details and allocator costs are separately scoped in the receipt. Reordering moves strings, restoration swaps rows in place; no second Row buffer. Existing 48 command, 192 byte text, 20 row/page and retained-surface/runtime workset ceilings remain unchanged.

No staging or commit. Fresh native-panels-target cache is terminal/available: 4.6 GiB, disk 27 GiB free. No other caches touched. Root-owned semantic/catalog/front-door files preserved.
