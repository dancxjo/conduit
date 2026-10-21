# Retained compositor core preparation

This dependency-free `no_std` crate is local preparation for ConduitOS #6177.
It is not integrated into the product or a protected image yet.

The caller provides two disjoint 2,097,152-element `u32` arenas. One is the
current retained raster store; the other holds a complete upload transaction.
Eight fixed surface descriptors use variable-size packed allocations. Preparing
an upload copies unchanged rasters in steps of at most 16,384 pixels. The new
raster arrives in ordered chunks within the same limit. Only a complete upload
can swap arena roles and publish its revision. Aborting preserves the current
raster. No equal surface partition, allocator or hidden pixel store is used.

Composition emits horizontal spans of at most 16,384 canonical RGB pixels.
Focus and cursor pixels follow the native compositor's frozen reference.
Damage merges touching rectangles and falls back to a union after 16 disjoint
rectangles. Per-step work is bounded. Native-style fixed scanline intervals
select the top surface once per span, bulk-copy retained pixels and fill focus
spans; only the small cursor overlay needs per-pixel masking. Conservative lifecycle damage can repaint more pixels than the optimized native
path. Completed coverage alone cannot enable advisory input routes. A separate
scanout-owner acknowledgement is required, and mutation invalidates old frame
tokens. Unchanged surfaces retain input readiness across partial and cursor-only frames; replacing a raster requires a
fresh positive scanout acknowledgement, and explicit frame/scanout failure
invalidates all input readiness. The Root client must validate all spans, copy
them successfully and correlate Host/Boot/service/display generations before acknowledging. It must still enforce
the authoritative checked Show and typed semantic interaction.

Handles, revisions and frame sequences are numeric. This Rust API is not a wire
protocol: Root pointers, slices and these in-memory structures must not be copied
into IPC. The service protocol still needs versioned integer/length messages,
instance-scoped upload operations and admission of the output/work bounds.
Restart, hardware protection, production presenter integration and the unchanged
graphical journey remain unproved. Tests use a test-only independent pixel oracle
adapted from `native_compositor/frame_composition_reference.rs`; that oracle is
not linked into the core.

Core revisions are renderer-local monotonically increasing upload sequences,
not semantic Face revisions. A repaint of the same Face therefore uses a fresh
upload sequence. The Root presenter retains exact Face, Mask and Show bindings;
the local sequence cannot establish any of those semantic identities.
