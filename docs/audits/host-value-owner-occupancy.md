# Inspect live ownership before discarding Host values

Development slice for #5231. Host-value discard previously scanned the entire
reserved pending-request and queue-slot arrays even when empty. It now uses
existing pending occupancy to skip an empty Host table and checks queue values
through each active cord's occupied ring prefix. No production state or
allocation was added. Nonempty pending-request inspection remains linear.

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
