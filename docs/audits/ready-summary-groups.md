# Skip empty readiness groups within the existing reservation

Development slice for #5231. The packed ready set previously inspected every
readiness byte in an empty span. Selecting node 9,999 from an otherwise empty
10,000-node set required 1,250 byte probes, as reproduced by the new regression.

For node capacities at least 64, the existing N-byte reservation now stores a
summary after the packed readiness bytes. One summary bit marks each nonempty
readiness byte. A missed readiness byte can skip eight empty bytes by inspecting
one summary byte. Summary membership changes only when a readiness byte changes
between empty and nonempty, so duplicate wakes and clearing one of several bits
do not duplicate or erase group membership. Clear resets both representations.

There are ceil(N/8) readiness bytes and ceil(ceil(N/8)/8) summary bytes; for
N >= 64 their sum fits comfortably inside the existing N-byte array. No field,
allocation, storage extent, scheduling class, or selection policy is added.
Capacities below 64 keep the former direct path. Iteration still observes the
same cursor-to-end then zero-to-cursor order and bounds partial active prefixes.
This is a one-level summary, not a universal constant-time selector: empty spans
still require O(N/64) summary probes and wakes maintain the summary.

The sparse 10,000-node regression now stays within 160 probes without increasing
the 10,000-byte object. It checks duplicate wake, last-member removal, mixed
byte/group boundaries, wraparound and clear. An independent linear oracle tests
all cursors for partial active prefixes around 8/64/128-node boundaries and 64
membership patterns. Existing small exhaustive membership and 256-node boundary
vectors remain. Production scheduler tests additionally cover loaded/sparse
pressure, Host completion, cancellation, fairness and Sign behavior under both
ready implementations.

This establishes deterministic selection-work improvement and local semantic
coverage. No wall-clock speedup, tiny-Host timing equivalence, full 10,000-node
Plan/Play run, or physical proof is claimed. The previous native timing refresh
covers capacities 1/8/32, which retain the direct path; it cannot measure this
large-set change. Parent #5231's production performance acceptance remains open.

Local validation: four fixed-only ready-set tests passed; alloc and
linear-ready-reference kernel suites each passed 200 tests. Formatting, strict
alloc all-target kernel Clippy, and diff whitespace checks passed. No CI was
dispatched or awaited.
