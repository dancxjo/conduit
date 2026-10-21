# #5628 Source preparation checkpoint

Worktree: `work/selected-typography-look`, branch `codex/5628-selected-typography`, base `58979c509`. No build, Cargo test, Clippy, cache, fetch, push or GitHub mutation performed. Source-only rustfmt, JS syntax checks and diff whitespace checks completed. Source was committed as 4f1263b054db98bd1f9d1b6f246332d83dce5071 and verified on origin; it is not yet compilation-proven.

## Delta

- Canonical required `VisualTypography::{Editorial,Engineering}`; exact expression/checked facet version2, aggregate Look version2, component Mask Fore2. No optional v1 defaults. Phosphor default explicitly Editorial; Todo compact/spacious roots explicitly select Engineering/Editorial independently of palette/density fields. Existing resource identities remain distinct from wire revisions.
- Six private immutable glyph profiles (Sans24 added beside Serif24), five text purposes. Same resolved `TextStyle` drives selected native ink/layout/measure/wrap/caret and compositor raster. Existing bounds unchanged, ensemble manifest records six profiles. Selected expression equality includes intent, invalidating retained raster reuse.
- Coherent native extraction: configured expression admission into `native_face_scene/expression.rs`; owner selection into `face_arrival/selection.rs`; bounded input preview into `input/preview.rs`. Façades/reducer remain below500 lines.
- Owner replacement retains exact named focus. Pending draft/submitted input refuses machine-readably before owner request or preference mutation. Desired accessibility preferences commit only at actual local Mask swap; no rollback claim after presentation/ack failure. Fresh authoritative Face presentation resets submitted state; stale unresolved occurrence cannot be replayed.
- Browser admitted-intent dataset and actual Title-purpose Host font stacks, preserving control fonts and checked actions. Prepared CPU mapping/ink/frame/focus/draft/cancel/raster tests and a pinned-browser typography-only correspondence case; all behavioral tests unrun.

## Exact compile-dependent regeneration

1. After root grants an exclusive cache, compile owning Face/native targets and fix type/ownership errors before claiming any proof. The changed checked Source must generate its real Native bindings.
2. Execute the owning `look_browser_binding::browser_profile_matches_checked_source_and_native_type_exactly` case with the existing `CONDUIT_LOOK_WIRE_FIXTURE` export pointing outside Source. It now exports actual checked value bytes and `<path>.type.hex` before comparisons. Old browser HEX/golden comparisons must still fail; this export does not bypass assertions. Preserve that failure log.
3. Install exported value bytes as a new `proof/conformance/face-look/phosphor-look-v2.hex`; replace browser `LOOK_TYPE_HEX` with exported semantic type bytes. Recompute its existing combined `types.conduit + newline + look.conduit` Source SHA256 pin. Never manufacture a nominal profile or overwrite the historical v1 golden.
4. Point current Rust/browser positive fixtures to v2; retain explicit v1 rejection cases. Change the Node invalid-version mutation from2 to1/3. Update exact expectation keys for returned typography. Then rerun the focused Face contracts/configured Mask/browser wire cases and owning native mapping/layout/raster/input checks.
5. Root reviews generated-byte and resource-ledger changes, then authorizes actual owner two-Look/browser/QEMU proof. No Source preparation or CPU fixture substitutes for that acceptance.

## Explicit remaining diagram boundary

`native_face_scene/diagram.rs` retains its default, unscaled `single_line_preview` for Heading. Both selected intents map Heading to identical Sans20, so the Title-only typography variation does not introduce a family mismatch there. This preparation does not prove all Patchbay text-scale paths or close #5629 accessibility/scale acceptance. Diagram scale debt remains explicit; no unrelated graph refactor is included.

## Separate-cache authorization

At 2026-10-21 01:18 UTC root measured 247 GiB free disk, 57 GiB available RAM, 20 CPUs and load 2.08. Focused compilation/regeneration is authorized in the separate `work/selected-typography-host-target` cache with four build jobs and a 100 GiB disk floor. The repaired589 full validation continues untouched in `work/native-panels-host-target`. No product/QEMU builds or full-workspace duplicate run authorized yet.
