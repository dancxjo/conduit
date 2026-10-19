# Prepared construction of exact mathematical data

Private preparation for #5392 reuses the accepted mathematical-value builder's
normalization, structural sharing, domain checks and canonical postorder codec.
`PreparedMathematicalValueBuilder` reserves the existing 32-node/depth-16
construction profile, canonical traversal scratch and a bounded encoding buffer
before execution. No Source syntax, Kind, wire version or arithmetic engine is
added.

The conservative retained-payload bound counts actual recorded vector capacities
and the container. It excludes allocator overhead, stack frames and caller-owned
output storage. Oversized or insufficient preparation limits refuse. Reset
retains capacity. Insertion checks the existing node/depth/work/domain laws;
encoding checks the root's current index/digest and uses the same canonical
writer as owning mathematical values. IDs are semantic value references, not
builder-instance authority tokens.

Encoding completes in private prepared scratch before copying to caller output.
Invalid roots, work exhaustion and short internal/external buffers leave caller
output untouched. Node/depth exhaustion, including partial tau expansion, can be
followed by explicit reset and valid construction without growth or a hidden
retry. No failed attempt authorizes emitting an older value.

Five tests include 96 repeated construction/reset/encode/borrowed-validation
cycles with zero observed allocations or reallocations. They retain identical
canonical bytes to the ordinary builder for π/180, principal sqrt(2), their
mixed expression, tau, cardinal sine and exact rational cancellation. Refusal
and recovery cases also observe no allocation and preserve caller sentinels.
The retained-payload limit is checked at its exact admission boundary.

Regression validation passes 106 Core library tests, the mathematical value,
enclosure, forward/inverse trigonometry and inverse writer tests, and the legacy
binary64 property fixture. Strict Core all-target Clippy, Core without default
features and formatting pass. No full workspace or target execution claim is
made; this is component evidence, with no throughput or WCET claim.

Canonical exact denotation and actual rounded computation remain distinct.
The builder normalizes `(2^53+1)-2^53` to exact one; the existing binary64
algorithm witness separately records an actual result of zero and error one.
This data graph cannot replace that operation-order witness.

The remaining runtime work includes an allocation-bounded numeric consumer of
borrowed symbolic data, checked Source/Plan/Play and certificate integration,
and the explicitly owned numerical projection boundary. Quantity wire profiles
currently admit decimal/rational coordinates, not a symbolic Quantity carrier.
No new public PR is opened during the Fore admission hold. All original parent
requirements and exact-source integration/publication gates remain in force.
