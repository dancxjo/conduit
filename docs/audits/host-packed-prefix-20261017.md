# Host adapter dispatch recognizes a packed occupied prefix

Local #5231 / #5395 implementation slice, 17 October 2026.

The previous active-node bound still examined 2,048 slots for a rejecting
adapter at cursor 1 when only slot 0 held a call. Dispatch can now use the
existing first-free allocation hint together with the retained-call count.

Every slot before `host_slot_search_start` is occupied. When that hint equals
`pending_host_count`, those earlier slots account for every retained entry,
so the tail is empty. The exact retained count includes dispatched calls and
completed calls awaiting consumption. This is an occupancy proof, not an
assumption that undispatched requests are contiguous. When equality does not
hold, dispatch retains the conservative active-node bound. No field or index
is added. Allocation, cancellation and completion do not gain new bookkeeping.

A cursor in the empty tail wraps directly to zero. Acceptance still advances
the cursor modulo the full reservation; rejection leaves it unchanged. The
sequence of predicate callbacks therefore matches the original full-table
cyclic order, including stateful predicates and competing adapters.

## Local evidence

The same release diagnostic on both implementations constructs a real scheduler
with 2,048 active nodes, 4,096 request slots and one retained request at slot 0.
Eight batches each perform 4,096 rejecting polls with the cursor fixed at 1.
Each poll invokes the predicate once; pending count, cursor and Signs stay
unchanged. Subsequent acceptance, empty polling and cancellation also succeed.

- Before: 8,388,608 slot probes per batch; observed batch wall 10.2–15.7 ms.
- After: 4,096 slot probes per batch; observed batch wall 15.6–16.1 µs.

These are instrumented microdiagnostic observations with assertions and test
scan counters, one process per variant, without affinity/frequency/load control.
They show removed empty-slot work; they are not full Plan/Play throughput or
an independently calibrated performance gate. The diagnostic fixture was added
to the base worktree before the baseline run. Exact production-file, fixture
and log hashes, raw batches, compiler and test receipts are retained in
`evidence/host-packed-prefix-20261017.json` and matching raw logs.

A real two-node completion/rearm/termination scenario leaves slot 0 empty and
slot 1 undispatched. All four cursors then match the independent full-table
adapter oracle, proving the packed-prefix optimization stops applying after
a hole appears. Existing oracle scenarios retain stateful predicates, competing
adapters, out-of-order completion and slot reuse. The thousands-of-calls
allocation/fairness/cancellation tests remain part of the full kernel suite.

Both alloc and linear-ready reference suites pass 240 local tests each. Strict
kernel Clippy, workspace formatting and the no-default-features check pass.
Tiny/dense elapsed-time regression and full hosted before/after measurements
remain separate acceptance work. The packed hint need not identify every
possible packed table after consumption; a conservative hint safely uses the
existing fallback. No CI requested; parent issues remain open.
