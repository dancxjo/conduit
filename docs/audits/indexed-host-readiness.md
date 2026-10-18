# Index pending Host Calls for ready selection

Development slice for #5231. A node now indexes its one canonical pending Host
Call through an optional nonzero u32 slot number. Ready selection, ready-work
inspection and context construction read at most that entry instead of scanning
the table. Request publication installs the index; completion consumption and
global cancellation clear it. Completion delivery leaves it installed until
consumption. Both preparation profiles initialize it. Request ordering and the
canonical table remain authoritative; no fallback scan or new scheduler exists.

The representation uses four bytes per reserved node and supports all 65,536
u16 node identities without colliding with the absent sentinel. The first-free
slot is bounded by the one-request-per-node contract; encoding rejects overflow.
A narrower optional u16 representation cannot preserve that entire range.

The 4,096-slot regression failed before repair: an unrelated ready node scanned
4,096 slots. It now reads no pending entries; a node with a live call reads one
entry in readiness and one in context. Fixed/hosted lifecycle fixtures compare
the index against actual table membership through cancellation, consumption,
re-arming and retirement. A two-node fixture checks separate slots, rejection of
a foreign request and out-of-order completion. Alloc/reference-ready suites each
pass 181 tests; 19 fixed-only Host tests and strict kernel all-target Clippy pass.

The [receipt](evidence/indexed-host-replay.json),
[raw evidence](evidence/indexed-host-replay-raw.tar.gz) and
[verifier](../../proof/performance/verify-indexed-host-replay.py) retain 64 paired
native debug/test replays against the preceding empty-table implementation.
Outputs, Signs, readiness, non-timing work and zero Play-allocation observations
agree. Native size growth at 1/8/32/256 reserved nodes is 0/32/128/1,024 bytes;
the one-node fixture uses existing padding. These are sampled native layouts,
not a portable memory or timing guarantee. No wall-clock speedup is claimed.
Adapter request selection, completion admission and other table queries remain
linear. Publication/consumption index maintenance is not separately timed.

After extracting the archive, rerun the verifier with `--before` pointing to
`host-empty-after.log`, `--after` to `host-index-replay.log`, their source text
files supplied through `--before-source`/`--after-source`, and a new JSON output.
Raw test logs classify the large-live-table and lifecycle evidence separately
from the empty-table replay. Parent acceptance remains open.


## Indexed completion admission

Completion admission now uses the same per-node slot index. It still checks
both the exact node and RequestId, dispatch state, absence of an earlier
completion, output bounds/value admission and Sign reservation before publishing
an outcome. The index is an acceleration of lookup, not permission to complete
a request. No production fields or allocation were added by this follow-up.

A 4,096-slot regression failed before repair: a wrong RequestId scanned all
slots. Admission now probes at most one entry for a node with a live request,
and zero entries for an unbound node or after global cancellation. The fixture
checks wrong IDs, the correct but undispatched request, duplicate completion,
valid dispatch/completion and late completion without unintended Signs or
pending-outcome changes. Alloc/reference-ready suites each pass 182 tests;
20 fixed-only Host tests, the final targeted assertion and strict all-target
kernel Clippy pass. A new test-only completion-probe counter is absent from
production layouts; prior replay size receipts remain source-pinned.

This is bounded-work and identity/refusal proof, not a measured wall-clock
speedup. Adapter request selection, completion consumption and other table
queries remain linear. Full #5231 acceptance remains open.


## Indexed Step preflight and completion consumption

Step preflight now borrows its node's canonical pending entry once, reusing it
for completion availability, cancellation correlation, duplicate/re-arm guards
and pending-membership checks. The existing occupancy count answers whether the
request table is full. Cancellation commit and completion consumption use the
same indexed slot, preserving exact node/request/completion checks and clearing
index and occupancy together. No production state or allocation was added.

Two strengthened work assertions failed before repair. The 4,096-slot fixture
now records zero preflight completion probes for an unrelated ready node and
one for a node with a pending call. The two-node out-of-order fixture records
one preflight entry and one consumed entry, rather than searching through the
other node's slot. Alloc/reference-ready suites each pass 182 tests; 20
fixed-only Host tests and strict all-target kernel Clippy pass, including
cancellation, re-arm, duplicate and Host value-alias ownership vectors.

New work counters exist only in test instrumentation. This is bounded-work and
semantic regression evidence, not a wall-clock speedup claim. Free-slot search,
adapter request/cancellation selection and other table queries remain linear.
Earlier replay/layout receipts retain their pinned sources. Full #5231
acceptance remains open.


## Retained obligations for selected nodes

`has_retained_host_call_for_nodes` now iterates the requested node set and uses
the per-node index, instead of scanning every reserved pending slot and testing
membership. It bounds-checks node IDs against the active prefix and retains an
exact node match. Empty and inactive sets remain false; completed outcomes still
count until consumption. This borrowed inspection neither consumes a request
nor certifies native idle. No production state or allocation was added.

The 4,096-slot regression failed before repair for an unrelated-node query.
Tests now require zero pending-entry probes for an unrelated node, one for a
retained call, and unchanged answers across undispatched/completed/cancelled
states. Fixed/hosted lifecycle fixtures compare the query with actual request
table membership through consumption and re-arming. Alloc/reference-ready
suites each pass 182 tests; 20 fixed-only Host tests and strict all-target
kernel Clippy pass. Work is proportional to the supplied node set, not an
unconditional constant; no wall-clock speedup is claimed. Other table queries
and full #5231 acceptance remain separate.


## Skip adapter polls with no undispatched requests

One bounded u32 count now tracks undispatched Host requests. Publication
increments it, accepting a request for dispatch decrements it, and global
cancellation clears it. Both preparation paths initialize it. A completion
cannot consume an undispatched request, so completion consumption leaves this
count unchanged. Pending/retained occupancy remains independent: dispatched
requests continue to exist while adapters work or outcomes await consumption.

Adapter polling returns immediately when this count is zero. With undispatched
work present, the existing cursor, predicate evaluation and dispatch ordering
remain intact; a rejected predicate does not decrement the count. The count
fits the one-pending-request-per-u16-node cardinality. It adds one u32 field per
scheduler, with target-dependent enclosing padding; no allocation was added.

The 4,096-slot regression failed before repair. It now checks zero slot probes
for empty and fully dispatched tables, predicate refusal followed by successful
dispatch, and independent retained occupancy. Fixed/hosted lifecycle oracles
compare the count with actual undispatched table entries through re-arming and
retirement. Alloc/reference-ready suites each pass 182 tests; 20 fixed-only Host
tests and strict all-target kernel Clippy pass. Polling with unmatched
undispatched work and free-slot search remain linear. This is work-bound proof,
not a fresh timing, layout or full #5231 acceptance claim.


## Skip cancellation polls without dispatchable obligations

A bounded u32 count tracks requested, undispatched cancellations whose Host
outcome has not arrived. Cancellation commit increments it; cancellation
dispatch decrements it. Accepted completion withdraws an undispatched
cancellation only after output and Sign admission succeed. Failed completion
therefore preserves it. Global retirement clears the count, and both
preparation paths initialize it. Underlying pending/retained request occupancy
remains independent. Existing cancellation selection order is unchanged.

The 4,096-slot regression failed before repair. The focused polling fixture
checks empty and ordinary pending tables, cancellation dispatch, completion
before cancellation dispatch, invalid-output refusal followed by cancellation,
and global retirement. Lifecycle oracles compare the count with actual
eligible table entries in fixed and hosted profiles. Alloc/reference-ready
kernel suites each pass 183 tests; 21 fixed-only Host tests and strict
all-target kernel Clippy pass, after integrating the current dev private-image
bound fix. No CI or physical acceptance is inferred.

This adds one u32 scheduler field, with enclosing padding unmeasured. Polling
with no dispatchable cancellation reads no pending slots. Selection with a live
cancellation remains linear, and no fresh wall-clock speedup is claimed. Test
fixtures share the existing cancellation builder through test-local visibility;
no production fixture or alternate runtime was added.
