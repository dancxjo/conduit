# Native Look surfaces and primary actions, 2026-10-20

Progress toward #5628 / #6174, following the bounded paint/antialiasing foundation a066837f9. This applies actual scene styling and changes the admitted editorial palette to the requested navy direction. It does not complete independent product selection, finite icon application, fallback/resource inspection or the full visual acceptance matrix.

The existing checked recipe and admitted density/text scale continue measuring the same Face. Panels now use Surface rounded fills and Border rounded outlines. Both commands are reserved before admission, including command-pressure continuation. Panel backgrounds are emitted before all row content: checked geometry ordering can differ from the restored semantic reading order, so a row-local background could otherwise overwrite an earlier row's ink.

Available primary actions, identified by their source-owned primary disclosure rather than their labels/identities, receive an Accent rounded fill, outline, and Background label ink. The existing heading-marker command is repurposed for the outline, so primary styling adds no row command. Secondary controls receive quiet rounded outlines; unavailable actions retain explicit wording and muted treatment. Forced colors preserve black label ink over white action fills and the existing non-color focus marker. Dark primary labels deliberately favor contrast over the reference illustration's white label.

Editorial uses navy background/inset surfaces, light body text, readable muted text and blue accents. Numeric test-only sRGB checks require text against surfaces and primary label/fill contrast at least 4.5, and outlines against surfaces at least 3.0, under both admitted palettes and forced colors. This is a finite palette check, not a claim to complete accessibility acceptance. The top accent bar now represents the actual local `(page + 1) / pages` position; it conveys no workload progress. The footer separator uses Border.

## Focused validation

122 tests passed, 2 existing manual performance observations ignored:

- `CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p conduitos --lib --features native-compositor native_face_scene::`: 96 passed, 0 ignored. Includes actual current owner Faces, complete Details, wide/narrow and 100–200% measured/rasterized text, canonical pages, source controls and typed Boolean/text interaction. New regressions cover available/unavailable primary disclosure across both densities/palettes/extents/scales and actual Crèche panel paint ordering within the fixed 48-command limit.
- Same built native unit binary, `native_face_mask::`: 11 passed, 0 ignored, including current-Show binding, failed scanout refusing Show publication, real checked Crèche recipes and long/empty/filtered names.
- Same binary, `native_compositor::`: 15 passed, 2 manual performance observations ignored, including actual palette contrast and incremental/fresh raster equivalence.

[Full logs and hashes](evidence/native-look-surfaces-20261020/). No new assets, dependency, command limit, semantic action/value, reading order, field-cluster policy or owner authority. #6158 remains explicitly skipped; no pagination policy fix is claimed. #6159 remains broader layout work and #5629 remains focus/accessibility acceptance.

Fresh source-pinned product capture is the next check. Older QEMU captures remain attributed to their original source. Full CI conventions and a PR are bypassed under the user's direct-dev instruction.
