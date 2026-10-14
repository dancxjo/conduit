# FixedScheduler scan baseline

For #5560, observe existing kernel traversals with test-only counters. Product
builds contain no counter field or increment. Selection order, eligibility,
pressure, routing and pending-call predicates remain unchanged.

The fixture uses fixed capacity 32 with active prefixes of 1, 8 and 32 nodes,
one real remote ingress Cord per active node, one queued byte per ready node,
one input Port and one empty pending-request slot. Loaded cases admit input to
every node; sparse cases admit only the last node after real startup Await steps.
Ingress wakes nodes through the normal kernel path; no readiness flag is edited.
A second input to the full single-item queue returns Full, never silent delivery.
Measured Back steps consume exact bytes transactionally and release all storage.
Loaded round-robin order is asserted. A one-node sparse case is the same workload
as its loaded control, and is retained explicitly.

Counters retain readiness nodes visited, readiness/context pending slots,
input Ports, Cord traversal entries and queue peeks. Sign count/bytes come from
the actual fixed Sign log. Independent expectations assert three full readiness
passes per active graph, two selected steps per admitted packet, one context
pending-slot check and input-Port check per selected step. Storage is empty at
the final Idle boundary. Full kernel conformance preserves existing fairness,
pressure, cancellation, terminal and remote lifecycle behavior.

The manual optimized test runs eight independently prepared trials per specimen,
with separate preparation and measured Step-to-Idle windows. CPU durations include
instrumentation, Back checks, Signs and the bounded fixture loop; they are not
unmodified product Step timings or provider service latency. Startup and ingress
are outside the Step window. Empty pending-table checks are measured; occupied
Host Calls are not inferred. There is no scheduler algorithm change, allocator,
SMP/preemption or synthetic Plan-cost claim, and no automatic timing threshold.

The broader #5537 requirements for allocation/copy attribution, independent clean
process repetitions and browser/WASM results remain separate. This baseline is
an actual native fixed-kernel byte fixture, not authored Source/Plan/Play or a
physical product performance run. Seventy nonmanual kernel tests and strict
Clippy pass. The optimized manual run remains pending.
