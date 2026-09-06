# Bounded addressable Resources

Owning architecture issue: #2583. Current acceptance remains the exact-main
record in `STATUS.md` and the issue; this document describes the contract.

Resource is the noun for bounded addressable content whose residence, lifetime,
sharing, access, generation, or durability outlives one ordinary inline Info
transfer. ResourceRef is still `value/resource-ref@1` Info. Records remain Info;
Forms and Gears are computations. State retains evolving Info across explicit
time boundaries; persistence materializes a Resource across a lifecycle boundary;
recording retains historical evidence. These do not introduce a `save` primitive.

The existing ResourceRequirement, ResourceOffer and ResourceBinding now carry an
optional content contract. Existing non-content pools keep their encoding and
identity. A content contract names exact semantic identity, version and content
profile, byte/item bounds, retention, sharing, access, generation slots, reader
leases, publication slots, and sensitivity. Offers add exact owner Host/Boot,
Base and residence profile. Bindings seal those facts into Plan identity. Neither
Form Info nor canonical ResourceRef encoding contains residence or handles.

Invocation, Play, Boot, Body-durable and externally durable obligations are
separate retention classes. The local provider in this slice implements only
Play retention and refuses other classes. Existing storage providers remain
responsible for durable operations; naming a durable obligation does not prove
that it was fulfilled.

Immutable-read-many and single-writer publication are supported. A read-only
requirement may select a published-generation offer without acquiring write
access. Synchronized mutable observation is explicitly unsupported. Generic
planning refuses a second writer for one pool; resource-owner admission also
refuses a competing writer without replacing an existing admission. Dynamic
shared-pool members cannot silently drop a content contract.

`HostedResourceGeneration` owns one exact pre-admitted generation. It uses the
existing kernel HostedValueStore for payload residency and reference counting.
Construction and reader installation occur before Play. Candidate bytes are not
readable until publication; published bytes are never exposed mutably. Fixed
reader slots and monotonic lease issuance prevent lease replay, and leases bind
resource identity, version and local owner/handle scope. Exact grants are checked
on every operation. The Host supplies admitted bindings; a ResourceRef is never
a grant. Retirement waits for readers, releases storage and makes the generation
lost. Another generation uses another sealed binding, leaving the old Plan and
any retained reference unchanged.

A shared-memory Line's internal queue or ring remains Line/Base machinery.
Resource residence is separate even when both mechanisms use memory. The
portable Patchbay projection discloses Resource meaning and version, access,
retention, sharing, bounds and exact owner/residence/Base under the existing
Gear realization inspection, while Cord inspection continues to disclose its
selected Line separately. The projection never changes the Plan.

## Decisive proof

Run `cargo xtask prove resource-frame --locked`.

The fixed source is source → compositor → display, with a second compositor →
encoder Cord. All Cords carry exact bounded ResourceRef Info. The ordinary
checker and planner produce two different exact Plans for the same checked and
expanded Form. Input/output resource generations, finite compositor scratch,
optional consumer materializations, authority and host operations are admitted
before the production kernel executes either Plan.

In the copy Plan, each consumer materializes the published output at its admitted
read boundary. In the shared Plan, both consumers read one payload residency.
The 256 × 256 × 4-byte frame is composed identically in both cases. The proof
counts three output payload residencies in the copy case and one in the shared
case, with identical checksums. It does not change the semantic Port/Cord type
between Plans or call a ResourceRef the payload itself.

A third placement moves display and encoder to a second Host while retaining
the local-residence requirement. The ordinary planner refuses that residence;
no remote dereference implementation, Line, pointer or DSM behavior is invented.

Additional proofs cover unreadable candidates, publication immutability,
authority refusal, finite lease exhaustion, stale leases, refusal to retire
while readers remain, candidate cancellation, exact subsequent generations,
State retaining ResourceRef Info, Plan identity changes, non-mutating inspection,
and zero allocations during publication/read-many/retirement after preparation.
These are deterministic contracts and hosted production-kernel execution,
not OS shared-memory, GPU/DMA, distributed-memory, browser-execution or physical
proofs. The frame composition is a named proof, not an installed general-purpose
compositor or a second runtime.
