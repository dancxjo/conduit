# Native role icons with actual antialiased product capture

Development progress for #5628/#5994 step10. Source
`c340c74260ec3209973ac96faf08f63bb02304ff` places the existing finite icons beside
primary subject labels whose explicit Face roles have an exact matching resource:
Body, Host, Gear, Port, Cord/Line, Plan, Play, Status and Diagnostic. Unknown/open
roles retain their complete text without inferred decoration. Names, guessed
application/action identifiers and arbitrary properties cannot select icons.
Details and action semantics remain intact; these decorations confer no control
or authority. Native owner independent density/palette controls from `54e8fe009`
are included in this product source, but this run exercises local Crèche selection.

The label inset is admitted before wrapping in ordinary flow, field-cluster
measurement and panel flow. Each icon costs one graphics command during page
admission, including continuation rows; the original 48-command/192-text-byte,
20-row and surface bounds remain unchanged. The small optional key uses the
existing bounded document/row storage, with no new allocation collection or
asset. Drawing reuses the retained 4×4 antialiased coverage path. Font coverage
filtering and measured typography remain unchanged.

## Checks and actual product evidence

- Two focused role/geometry checks passed: explicit roles versus unknown/action
  roles, reserved text space, non-overlap and containment at 100/125/200% scale.
- All 99 native scene regressions passed, zero failed/ignored. This includes
  the maximum Todo matrix with four profiles × two recipes at forced colors,
  200% text and 320×240, complete Face/item text and all 40 available controls.
- Formatting and diff checks passed.
- On clean unchanged source `c340c7426`, normal product build and canonical
  `make host verify --journey` both exited 0. Final ISO SHA-256:
  `f8ea1d02612d8f8db05ce2fa85d2553eb2ff46c6e0b4ef5ce773b61856612d63`.
  Its guest resolved-description image binding is separately
  `image:sha256:55c9a8bf81089e93c15c9449ff9daa30faa7cf3d126755e802ff762254a4cc31`.
- All 26 actual QEMU captures passed: four real keyboard Look transitions plus
  the original 22 interaction/lifecycle/Home/Patchbay checkpoints. The transitions
  preserve the complete Face, checked component and canonical recipe with fresh
  Shows and zero Body actions. Commands are 48/25/25/48, within the unchanged 48.
- All 26 PNG sizes/file hashes and decoded RGBA hashes were independently checked
  against the run's original capture manifest. Only manifest-bound PNGs were
  retained, excluding any stale frame from the shared build directory. Visual
  inspection of the actual spacious editorial frame confirmed the Host icon
  separated from its serif heading and smooth enlarged text and rounded surfaces.
- All three generated typography files match the previously accepted hashes
  byte-for-byte, including the 761,651 coverage bytes. Runtime typography assets
  remain 880,471 bytes; generated Rust source size is a separate quantity.

Commands used (incremental caching disabled only to control local disk use):

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output /home/dancxjo/Documents/Codex/2026-10-20/goal-bypass-the-standard-proof-and/work/5628-semantic-icon-product-3/image --locked
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo xtask make host verify /home/dancxjo/Documents/Codex/2026-10-20/goal-bypass-the-standard-proof-and/work/5628-semantic-icon-product-3/image --journey --locked
```

[Complete receipts, original manifest and captures](evidence/native-semantic-icons-20261020/)
retain source/build identities and a separate hash ledger. The first build
failed at the shared bare-metal compile check with disk exhaustion; the second
failed linking the host tool while disk filled, truncating its diagnostic log.
Neither is passing evidence. After verified inactive cache cleanup, the third
build and its subsequent journey passed on the same unchanged source. Cleanup
preserved linked binaries/libraries, release/images, source and captured evidence;
other workspaces' active compilers were left alone.

## Remaining scope

This proves actual local product icon scanout and independent Crèche Looks,
not a fresh live owner Todo appearance capture, attended human use, physical
hardware, performance improvement or stable publication. #5628 and #5994 step10
remain open pending live owner appearance and remaining fallback/resource
acceptance. Accessibility/focus certification and the complete native product
matrix remain their separate following steps. #6158/#6055/#6155 remain skipped.
