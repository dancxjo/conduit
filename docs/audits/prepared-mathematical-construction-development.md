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
The local Linux x86_64 capture pays five preparation allocations and retains
100,259 bytes of accounted payload for the full 33,059-byte encoding profile.
This fixed worst-case reservation is substantially larger than the specimen
values; it is optional preparation cost, not a whole-process memory measurement.

Regression validation passes 106 Core library tests, the mathematical value,
enclosure, forward/inverse trigonometry and inverse writer tests, and the legacy
binary64 property fixture. Strict Core all-target Clippy, Core without default
features and formatting pass. No full workspace or target execution claim is
made; this is component evidence, with no throughput or WCET claim.

Canonical exact denotation and actual rounded computation remain distinct.
The builder normalizes `(2^53+1)-2^53` to exact one; the existing binary64
algorithm witness separately records an actual result of zero and error one.
This data graph cannot replace that operation-order witness.

The companion [borrowed enclosure preparation](borrowed-mathematical-enclosure-development.md)
adds an allocation-bounded Core consumer of borrowed symbolic data. Remaining
runtime work includes checked Source/Plan/Play and certificate integration,
and the explicitly owned numerical projection boundary. Quantity wire profiles
currently admit decimal/rational coordinates, not a symbolic Quantity carrier.
No new public PR is opened during the Fore admission hold. All original parent
requirements and exact-source integration/publication gates remain in force.

## Borrowed operand import (private continuation)

The prepared builder can append an already validated `MathematicalValueView`.
It remaps the operand's local references through a fixed 32-entry mapping and
uses the existing checked insertion, structural interning, node/depth ceilings,
and limb-work meter. An index from one operand cannot identify an unrelated node
in another. Importing a shared graph twice retains the same destination nodes.
No floating-point execution witness or physical/source authority is inferred.

The input remains borrowed; import allocates no buffers. A refused import may
retain a partial construction, so the caller resets before starting another
transaction. Result encoding retains its separate no-partial-output law. The
selected builder suite has seven passing tests, including 32 guarded joins of
separately encoded Pi/180 and sqrt(2) operands with zero observed allocations,
unchanged retained capacity and byte equality with ordinary Core construction.
Node/work refusals followed by reset and recovery are also guarded.

At this import-only checkpoint, Source arithmetic still accepted rational
operands only. The subsequent private integration is recorded in
[symbolic Source arithmetic](symbolic-source-arithmetic-development.md). Import
alone establishes neither numerical projection nor parent acceptance.
