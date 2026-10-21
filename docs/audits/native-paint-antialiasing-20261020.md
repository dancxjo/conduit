# Native paint and antialiasing foundation, 2026-10-20

This is a bounded prerequisite for #5628 and the #6174 visual reference, including the user's explicit request to antialias graphics alongside fonts. It does not complete the selected Look, component application, focus work, or the real product visual comparison.

## Reuse and missing paint

#3164 already supplied the finite 23-icon vocabulary and tokens. #3049 supplied rounded rectangles. The retained graphical renderer already supplied grayscale text coverage. Reuse those mechanisms: no CSS parser, font parser at presentation time, external icon package, new image asset, or separate theme engine.

The existing Background/Foreground/Accent/Muted roles did not independently describe inset surfaces and their outlines. Append Surface=12 and Border=13 to the existing one-byte paint form. Existing tags 1–11, graphics version 2, command storage and encoded offsets remain intact; older decoders refuse the unknown roles. New decoders continue refusing unsupported tags. Palette and platform forced-color policy resolve these roles in the existing admitted expression path. This change does not select a new editorial palette.

## Edge coverage

The retained graphical scene renderer now draws rounded fills, rounded outlines and semantic icons with a fixed 4×4 subpixel grid. Icon line/ring geometry uses eighth-pixel integer coordinates, shared with the write-only renderer. Rounded geometry tests the original bounds, so clipping cannot invent a new outline. Square rectangles and orthogonal paths retain their aligned integer edges.

Each visible pixel performs at most 16 membership tests, then at most one retained-background read and one write. Zero coverage performs neither. RGB channels blend against the actual retained pixel with rounding; unused framebuffer bits survive. Geometry has no heap allocation, runtime-sized coverage buffer, persistent cache, or floating-point requirement. This increases edge raster arithmetic and makes no speedup claim. Existing command, text, surface and typography asset limits are unchanged. The write-only rescue path remains binary because it cannot read the background for blending.

## Validation and limits

Focused checks passed on the changed source over base `779ab7f1d`:

- `CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p conduitos --lib --features native-compositor native_compositor::`: 15 passed, 2 existing manual performance observations ignored. Includes incremental-versus-fresh pixel equality under edits, ordering, clips and blending; revision fences, refusal and allocation checks.
- The same freshly built test binary, `display::` filter: 33 passed, 1 existing manual volatile-scanout performance observation ignored. Includes font coverage/layout and write-only rounded clipping.
- `CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p conduit-face --lib graphics::tests`: 6 passed, 0 ignored. Covers unchanged existing paint tags, new roles, unsupported-tag refusal and unchanged command encoding size.

The new smooth geometry check exercises all 23 icons and both rounded styles at 12×10, 24×20 and 28×22 extents. It requires intermediate blended pixels, preserved unused bits, bounded writes and exact equality between full and clipped render regions. Separate admitted-palette tests cover both palettes and forced colors. Full logs and their hashes are retained in [the evidence directory](evidence/native-paint-antialiasing-20261020/).

An initial build exhausted local disk and did not validate the change. The successful runs followed disposable package-cache cleanup; no source or accepted image/capture was removed. Full CI, release performance observations and a new QEMU visual capture were not run for this foundation. Direct `dev` integration and bypass of standard CI/proof conventions follow the user's explicit instruction. Remote updates through `58979c509` affect separate browser/lifecycle proof fixtures; the changed graphics, font and compositor source remains identical after rebasing. Existing accepted typography and QEMU captures belong to source 6a8e127b4; they are not captures of this graphics change. Full selected-Look native product acceptance remains part of #5628.
