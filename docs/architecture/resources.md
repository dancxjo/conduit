# Bounded addressable resources

Owning architecture issue: #2583. Current acceptance remains the exact-main
record in `STATUS.md` and the issue; this document describes the contract.

resource is the noun for bounded addressable content whose residence, lifetime,
sharing, access, generation, or durability outlives one ordinary inline info
transfer. ResourceRef is still `value/resource-ref@1` info. Records remain info;
forms and gears are computations. state retains evolving info across explicit
time boundaries; persistence materializes a resource across a lifecycle boundary;
recording retains historical evidence. These do not introduce a `save` primitive.

The existing ResourceRequirement, ResourceOffer and ResourceBinding now carry an
optional content contract. Existing non-content pools keep their encoding and
identity. A content contract names exact semantic identity, version and content
profile, byte/item bounds, retention, sharing, access, generation slots, reader
leases, publication slots, and sensitivity. Offers add exact owner host/boot,
base and residence profile. Bindings seal those facts into plan identity. Neither
form info nor canonical ResourceRef encoding contains residence or handles.

Invocation, play, boot, body-durable and externally durable obligations are
separate retention classes. The local provider in this slice implements only
play retention and refuses other classes. Existing storage providers remain
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
Construction and reader installation occur before play. Candidate bytes are not
readable until publication; published bytes are never exposed mutably. Fixed
reader slots and monotonic lease issuance prevent lease replay, and leases bind
resource identity, version and local owner/handle scope. Exact grants are checked
on every operation. The host supplies admitted bindings; a ResourceRef is never
a grant. Retirement waits for readers, releases storage and makes the generation
lost. Another generation uses another sealed binding, leaving the old plan and
any retained reference unchanged.

A shared-memory line's internal queue or ring remains line/base machinery.
resource residence is separate even when both mechanisms use memory. The
portable Patchbay projection discloses resource meaning and version, access,
retention, sharing, bounds and exact owner/residence/base under the existing
gear realization inspection, while cord inspection continues to disclose its
selected line separately. The projection never changes the plan.

## Decisive proof

Run `cargo xtask prove resource-frame --locked`.

The fixed source is source → compositor → display, with a second compositor →
encoder cord. All cords carry exact bounded ResourceRef info. The ordinary
checker and planner produce two different exact plans for the same checked and
expanded form. Input/output resource generations, finite compositor scratch,
optional consumer materializations, authority and Host Calls are admitted
before the production kernel executes either plan.

In the copy plan, each consumer materializes the published output at its admitted
read boundary. In the shared plan, both consumers read one payload residency.
The 256 × 256 × 4-byte frame is composed identically in both cases. The proof
counts three output payload residencies in the copy case and one in the shared
case, with identical checksums. It does not change the semantic port/cord type
between plans or call a ResourceRef the payload itself.

A third placement moves display and encoder to a second host while retaining
the local-residence requirement. The ordinary planner refuses that residence;
no remote dereference implementation, line, pointer or DSM behavior is invented.

Additional proofs cover unreadable candidates, publication immutability,
authority refusal, finite lease exhaustion, stale leases, refusal to retire
while readers remain, candidate cancellation, exact subsequent generations,
state retaining ResourceRef info, plan identity changes, non-mutating inspection,
and zero allocations during publication/read-many/retirement after preparation.
These are deterministic contracts and hosted production-kernel execution,
not OS shared-memory, GPU/DMA, distributed-memory, browser-execution or physical
proofs. The frame composition is a named proof, not an installed general-purpose
compositor or a second runtime.
