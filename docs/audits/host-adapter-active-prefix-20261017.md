# Host adapter dispatch skips unreachable reserved slots

Local #5231 / #5395 implementation slice, 17 October 2026.

A scheduler with two active nodes and 4,096 reserved pending-call slots could
inspect all 4,096 slots when a rejecting adapter started at cursor 1 and the
only undispatched call occupied slot 0. Request allocation always chooses the
first free slot; each active node owns at most one retained request, including
completed requests until consumed. Therefore an allocation cannot reach index
`active_nodes`: before allocating, at most `active_nodes - 1` slots are occupied.
Completion and cancellation only remove entries; reuse preserves the bound.

Dispatch now traverses at most `min(active_nodes, PENDING_REQUESTS)` slots. A
cursor in the unreachable reserved tail wraps directly to zero. The externally
retained cursor is still updated modulo the full reservation after acceptance
and remains unchanged after rejection. Eligible callback order and stateful
predicate behavior therefore match the original cyclic full-table traversal.
No runtime field, per-request storage, allocation, Sign or policy is added.

## Local diagnostic

The same release test on either implementation performs eight batches of 4,096
rejecting polls over a real initialized scheduler. Every poll sees the same
request exactly once, leaves cursor 1 unchanged, retains the pending count and
records no extra Signs. Subsequent dispatch and cancellation release values.

- Before: 16,777,216 slot probes per batch; observed batch walls 6.3–8.3 ms.
- After: 8,192 slot probes per batch; observed batch walls 20.4–20.7 µs.

Raw batch measurements, compiler, host model, exact source/fixture hashes and
command are in `evidence/host-adapter-active-prefix-20261017.json`. This is one
process per variant, with no affinity/frequency control, and includes test-only
counters, assertions and `black_box`. It demonstrates eliminated empty-slot
work, not a calibrated speedup of ordinary Plan/Play or a regression threshold.
Both tiny and fully occupied reservations retain their original search domain.

Existing full-table adapter oracle tests cover all four cursors, stateful
predicates, competing adapters, out-of-order completion and slot reuse. Focused
probe assertions cover prefix, wrapped-tail and in-prefix cursors. Kernel tests
are run with alloc and with the linear-ready reference; strict kernel Clippy
checks all targets. Parent issues and broader hosted acceptance remain open.
