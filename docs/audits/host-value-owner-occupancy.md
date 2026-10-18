# Inspect live ownership before discarding Host values

Development slice for #5231. Host-value discard previously scanned the entire
reserved pending-request and queue-slot arrays even when empty. It now uses
existing pending occupancy to skip an empty Host table and checks queue values
through each active cord's occupied ring prefix. No production state or
allocation was added. The follow-up below bounds nonempty pending inspection
by the last occupied slot; holes before it still require inspection.

Ring positions preserve logical order across wraparound, without overflowing
a u16 head-plus-offset. Pending input/output references and occupied queues
still refuse discard as `ValueOwned`; unowned temporary values still use the
storage profile's ordinary release checks. Inactive and unused reserved queue
slots carry no ownership. The query walks active cord headers plus occupied
entries, which is not a universal constant-time or wall-clock improvement.

The 4,096-slot regression failed before repair. Tests check zero slot probes
for empty reserved tables, ordinary temporary-value release, and preservation
of every wrapped/coalesced queued reference after superseded values are
released. The existing Host alias and completion-ownership vectors also pass.
Alloc/reference-ready suites each pass 185 tests; 23 fixed-only Host tests and
strict all-target kernel Clippy pass. These are local kernel/work-bound proofs,
not new transport, physical or release acceptance. Parent #5231 remains open.


## Nonempty pending-table tail

The follow-up regression measured 4,096 pending-slot probes when releasing an
unowned temporary while one request occupied slot zero. The ownership query now
counts down the existing live pending count and stops after visiting that many
occupied entries. Completed requests awaiting consumption remain in that count.
No new production counter, index, allocation, or ownership state is introduced.

A fixed-storage two-node fixture exercises real request dispatch, completion,
and consumption in a 4,096-slot reservation. Unowned cleanup inspects one slot
with one request, two with two requests, two after the first request is consumed
and leaves a hole, and zero after both are consumed. The remaining request's
input and its staged completion output both refuse discard. Ordinary consumption
reclaims all values. The regression fails against the former tail scan (4,096
versus one probe) and passes after the change.

This is deterministic local work evidence, not a wall-clock speedup. Worst-case
inspection is still linear through the highest occupied slot; the query also
checks active cord headers and occupied queue entries when no pending owner is
found. Parent #5231's broader measurement and acceptance requirements remain.

Follow-up validation: alloc and linear-reference kernel suites each passed 189
tests; the focused ownership regression also passed without alloc. Formatting,
strict alloc all-target kernel Clippy, and diff whitespace checks passed locally.
