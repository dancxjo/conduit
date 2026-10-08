# Numeric local retained inventory

AI fixed numeric codecs, tensor port bindings and prepared Back owners expose
`local_accounted_heap_bytes()`. The methods walk actual retained Vec/String
capacities without allocation, include spare array slots and owned access IDs,
and retain existing conservative upstream evaluator accounting. They exclude
the boxed Back root, shared tensor/model Arc owners and headers, allocator
bookkeeping and stack. Callers must charge/deduplicate those separately.

The full tensor descriptor helper includes both bounded sequence capacities,
each live axis identity/custom role String, inline boxed payload or complete
resource-reference identity/clock-basis metadata. Resource owners expose exact
descriptor sharing and separate descriptor/access payload inventories; the
existing complete backing-storage sharing API is preserved. No tensor, law,
resource authority or runtime arithmetic changed.

Three actual coherent-SDK tests pass: all three codec classes equal actual
retained requested allocations; the original archived Plan's float-to-i16 Back
has 6,392 local heap bytes plus a 2,672-byte Box root, matching 9,064 retained
allocation bytes; a descriptor with spare sequence storage/custom axis strings
has exactly 372 heap bytes. Getter probes allocate zero. These probes do not
claim all Back variants have independent allocator coverage.

AI no-default-features library build and Clippy `-D warnings` pass. Source and
all artifact hashes are retained with a content-addressed, compiler-closed SDK
snapshot (24 runtime rlibs plus exact serde_derive.so), not mutable Cargo paths.
Evidence is in the goal project's outputs/fargan-numeric-retained-inventory.

This is retained local accounting, not generic preparation admission or a
complete concrete driver inventory. Whole public trained execution and
#5212/#5215/#5218 acceptance remain separate.
