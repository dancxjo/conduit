# Selected typography reconciliation audit

Read-only comparison: current dev `c4c10b3e38bc0d217e001f980f65a642dfe82280`, held `ba44c5cb3ddf9447b46349346d16ee490ec9a173`, common base `0d5530bc9fd0cfc29f3548ce28e93e620814173b`. No Source edits, builds, fetch, push, guests or GitHub observations. Legacy three-way `git merge-tree BASE DEV HELD` inspected without modifying a worktree.

## Smallest coherent strategy

Keep current dev as base and union the held selected-typography delta; root's isolated `selected-look-reconciled` trial merge is appropriate. Do not cherry-pick cedf/76e6/56d2/785 again, replace dev files wholesale with held blobs, or cherry-pick the held eleven intermediate repair commits one-by-one onto dev. Preserve original ba44 and all old source-pinned receipts; final reviewed merge/squash must describe only the resulting v2 typography/owner-swap delta. No compatibility decoder or extra widget/Look framework is needed.

There are10 shared changed paths, but only3 textual conflict files: `targets/conduitos/src/display/typography.rs`, `display/typography/scene.rs`, `native_face_scene.rs`. The remaining overlaps auto-merge textually and still need semantic review: both authored component files, `semantics/face/src/component_recipe.rs`, `display/typography/raster.rs`, `native_compositor/raster_reuse/tests.rs`, `native_face_scene/input.rs`, `product_front_door/face_arrival.rs`.

Resolve typography.rs as a union: keep dev `mod coverage`, `GlyphRaster::base_coverage`, 100% identity sampling and four-tap scaled reconstruction/tests; retain held selected Typography/TextStyle/metrics/glyph APIs and sixth Engineering Title resource. In scene.rs retain selected-purpose resolution for every Text command plus current formatting; do not restore old unconditional role-only paint. In native_face_scene.rs retain dev encounter module/state-transfer collaborators AND held selected-style/expression/layout collaborators. Retain raster's new selected entry point plus dev filter behavior/comment. Retained-raster tests must keep independent expression invalidation and dev filtering tests; measurement/paint must choose the identical profile.

## Auto-merge hazard: ten appended roots

Dev `ComponentVisualSelection` supplies a2×2 density/palette matrix and6 additional Crèche/4 Todo wrappers. Held changed older expressions to exact v2; the appended wrappers still contain v1/missing typography after naive merge. Upgrade **every** authored VisualExpression to v2 with an explicit intent, retaining root names, recipes, density, palette and scale. Recipe schema remains1; do not mechanically change recipe versions.

Preserve held original Crèche single/columns `Editorial` even with Phosphor palette: that intentionally keeps existing Serif Title. Preserve original Todo compact `Engineering` and spacious `Editorial`. For appended6Crèche+4Todo wrappers, the least semantic change is explicit **Editorial**: before reconciliation their fixed Host Title was Serif regardless of palette. Thus spacious-phosphor remains Editorial typography+Phosphor palette. Do not infer Engineering from Phosphor, nor conflate the palette name Editorial with typography. A different authored choice is a separate deliberate behavior change with its own expectation/capture, not historical proof reuse.

Retain dev selection.rs root matrix and component_recipe.rs reexports while retaining held component-Fore revision2. Recompute existing types+look browser Source pin only after final authored/schema bytes freeze; regenerate current v2 fixtures through producer entrances, preserving exact-v1 refusal fixtures. No v1 widening/default typography.

## Keep both interaction paths

Dev `face_arrival/look_selection.rs` is explicitly local Crèche: it prepares first, transfers pending encounter/focus via `preserve_encounter_from`, suspends then presents, and sends no Body action. Keep it, its tests and encounter modules. Held `face_arrival/selection.rs`/owner_selection.rs is authenticated owner Todo replacement: pending owner draft refuses locally with old Show/draft retained; successful replacement acknowledges owner Show and then records retained named focus. These scopes are distinct. Preserve local dispatch in `face_arrival/input.rs` and owner dispatch in front-door routing; no key should execute both. Dev input `submitted` visibility is required by encounter transfer; preserve it alongside held has_pending_input.

## Evidence boundaries / next safe step

Dev cedf/76e6/56d2/785 logs and26 actual captures establish local four density/palette transitions, exact Face/component/recipe conservation, Show changes, draft transfer and filtered enlarged text on their exact recorded source/image. Keep them unchanged. They do not prove independent Sans-vs-Serif Title selection, authenticated Todo owner swap, or the reconciled source. Held CPU proof supplies typed independent purpose and owner refusal/focus laws; actual owner proof remains outstanding.

Next: integration owner resolves the3 conflicts, explicitly upgrades10 appended expression roots, reviews both dispatch paths and the component identity invariants, then freezes a new source for focused conformance and source-bound product/owner proof. No fresh build/capture is justified before that review. Current dev's source-preserved historical captures must never be relabeled as final reconciled proof.
