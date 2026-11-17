# Exact Plan-to-kernel lowering

The existing fragment and complete-Plan entrances share one materializer. They
preserve authored placement/port/connection ordinals, canonical route ordering,
complete Plan verification and original rich identity correlations. Local
fragment-set composition rebases those same tables without inventing a Plan.

For a funded hosted preparation, first call
`lower_local_fragment_set_layout_from_plans` with borrowed complete Plans,
the selected storage profile and the existing fragment-set limits. This performs
checked shape accounting without allocating. It does **not** verify the seals
or grant execution. Reserve `FragmentSetLayout::peak_owned_bytes` on the original
Host memory counter before calling
`lower_local_fragment_set_from_plans_with_limit` with that same allowance. A
one-byte shortage refuses before verifier scratch or any new owned allocation.
The materializer still calls the ordinary complete-Plan verifier.

The highwater contains every final partition/table and identity String owner
coexisting, plus the maximum complete verifier/refusal scratch. Vectors reserve
explicit capacities fallibly; placement/port/connection lookup uses borrowed
linear scans rather than allocating trees. Routes retain source ordering and
fan-out target order through the same generic lowering. No allocator-node-size
multiplier, new semantic ceiling or second kind-specific lowerer is used.

`lowered_local_fragment_set_backing_bytes` inspects actual final String/Vec
capacities without allocation. Check it against `retained_owned_bytes` before
Started; it measures retained drift, not temporary verifier highwater. These
are requested Rust backing extents, not process RSS, physical confinement or a
general recoverable system-OOM promise. Existing String clones and unsupported
reference-profile owners retain their normal allocation failure behavior.

The funded descriptor currently refuses State, fusion, shared pools, execution
regions, remote/selected Lines, resource-transfer cords, terminal profiles and
nested resource/Fore constraints. Fore realization-property decoding also needs
its own bounded ownership qualification. Ordinary existing entrances still
support their previously admitted shapes through the same implementation; an
unsupported funded descriptor must not cause fallback to an unfunded path.

The `bounded_local_layout` owning test target checks allocation-free layout and
shortage, actual allocator highwater, retained capacity inspection, full-table
reference/funded equality, explicit fan-out ordinals and multi-partition rebasing,
Fore identities, unsupported shapes, arithmetic overflow and invalid seals.
Existing owning lowering tests remain necessary for State, remote, terminal,
resource, shared-pool and coalescing semantics. This is hosted CPU preparation
proof; it establishes no actual Body Started, device or protected execution.
