# Native focus and overflow navigation (#5607)

The native Mask retains the complete canonical Face. Tab and Shift-Tab traverse
available action/argument controls in source reading order across page boundaries.
Pointer selection and F2 inspection retain the exact control when it occurs in
the destination view. Enter still crosses the acknowledged Show boundary; stale
Shows and unavailable actions refuse there without changing owner truth.
A stale scene input repaints the bounded “Stale: refresh” notice and preserves
the machine-readable refusal; the notice neither replaces the Face nor creates
a new semantic Show.

Focus uses two geometrically distinct outlines, so selection is legible with
identical colors. Primary reading includes unavailable and refused actions with
their explicit owner reason code and explanation. Details retains complete
relationships, properties, argument contracts and provenance. PgUp/PgDn reads
all admitted text, including wrapped continuations; the header reports the
current page and total. Geometry and graphics command pressure paginate rather
than silently discard content.

`cargo xtask check input-semantics` includes the native scene
navigation proof: forward/backward keyboard traversal, pointer-to-keyboard
inspection, complete overflow reading, distinct refusal wording and unchanged
Face identity. It exercises 320×240, 640×480 and 1280×800 extents alongside the
existing measured-layout, enlarged-text, stale-Show and input contract tests.
The stale-input case also runs through the production Mask/compositor with a
counting pixel adapter and verifies actual writes. These are deterministic
production-layout/input and adapter tests, not QEMU scanout,
physical hardware or human usability evidence. Candidate and any required
stable acceptance remain separate receipts.
