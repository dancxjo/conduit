# Bound batch cancellation polling across thousands of Host calls

Development slice for #5231. Cancellation dispatch previously searched the
pending table from slot zero for each notification. A production regression
with 2,048 live calls reproduced 2,098,176 slot probes for one cancellation batch.

The scheduler now maintains one usize search bound. Every eligible cancellation
below it has already been dispatched or settled; a newly committed cancellation
lowers the bound to its slot when necessary. Successful dispatch advances past
that slot. Fixed/boxed construction and whole-run cancellation initialize/reset
the bound. Completion can remove eligibility without creating any earlier work.
This preserves the former lowest-slot dispatch order, including newly rearmed
requests in earlier slots. It adds one fixed scalar field and no allocation.
A single poll can still scan a sparse span; repeated rewinds can still revisit
slots. This is not a universal constant-time cancellation queue.

## Real stress flow

The native fixture admits 2,048 nodes, ingress cords and Host bindings, a
4,096-slot pending table, finite value storage and 32,768 Sign slots. Ordinary
remote control values wake the waiting Backs to request cancellation; no ready
flags or pending entries are seeded by the test. Initial request allocation
uses exactly 2,048 slot probes.

Two full rounds dispatch cancellation in original slot order, complete calls in
reverse node order, and resume them fairly. Outcomes alternate between completed
and cancelled; completed outputs share explicitly retained references to one
bounded value. The first resume consumes completion and rearms a new request in
the same Step; its next cancellation must rewind to the earlier reused slot.
Each batch now takes exactly 2,048 cancellation probes instead of 2,098,176.
Stale and duplicate completions refuse, each accepted completion wakes its exact
node, and indexed completion admission/consumption visits one live slot per call.
All output references and the original placeholder are reclaimed at the end.

A second scenario globally cancels after all reverse completions arrive but
before Backs consume them. It verifies driver cancellation, drained pending
indices, no later dispatch and complete value reclamation. Both scenarios
allocate/reallocate zero times across the request/control/cancel/complete/resume
phase, including assertions. Thread creation and preparation are outside that
sample; formatting occurs afterward.

Each run checks contiguous Sign sequence numbers, exact Host request,
cancellation and reverse-completion identities/order, exact remote lineage links,
and no retention gap. The normal scenario retains 32,768 Signs; global cancellation
retains 14,338. The packed and linear selector runs produce identical normalized
work/terminal rows. This is local fixed-kernel conformance, not an authored
Plan/Play journey, optimized throughput or physical-device proof. The large fixed
fixture uses a 64 MiB diagnostic thread stack.

Local validation: focused fixed-only stress runs passed with both selectors;
alloc and linear-ready-reference kernel suites each passed 210 tests, including
existing cancellation/completion races. Formatting, strict alloc all-target
Clippy and diff whitespace passed. Parent #5231 remains open. No CI was
dispatched or awaited.
