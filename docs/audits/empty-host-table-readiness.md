# Skip empty Host Call tables during ready selection

Development slice for #5231. Ready selection, ready-work inspection and Step
context construction previously traversed reserved pending Host Call slots even
when the table was empty. One bounded occupancy count now lets these paths
skip the empty table. Counts increment on request publication, decrement when
a completion is consumed, and clear with global cancellation; completed but
unconsumed requests still occupy their slot. Both ordinary and boxed
preparation initialize the count, and the public occupancy query uses it.

The new zero-probe characterization failed before repair. Existing fixed and
hosted cancellation/re-arm fixtures now compare cached occupancy with the
actual request table through the lifecycle; global cancellation independently
checks complete clearing. Alloc and linear-ready reference suites each pass
178 tests; four fixed-only Host cancellation tests and strict all-target kernel
Clippy pass. Live pending-table lookup behavior and request ordering are unchanged.

The [receipt](evidence/empty-host-table-replay.json),
[raw capture](evidence/empty-host-table-replay-raw.tar.gz) and
[verifier](../../proof/performance/verify-empty-host-replay.py) retain 64 paired
instrumented native replays: 1/8/32/256 nodes, sparse and loaded, eight trials.
Exact outputs, Signs, remote evidence, readiness, value accounting and other
non-timing work agree. Ready/context pending-slot probes fall from 9,632 to zero;
observed Play allocations remain zero. The sampled native scheduler adds eight
bytes for its one `usize` count. Other target layouts are unmeasured.

These are debug/test work and replay observations, not a wall-clock speedup,
production throughput or physical acceptance claim. Live Host Call selection
and other pending-table queries remain linear. Host request publication and
completion consumption now maintain the count; their runtime cost is not
separately timed here.

To reproduce each pinned source, run the command retained in the raw metadata.
After extracting the archive, run the verifier with `--before` and `--after`
pointing to the two logs, `--before-source` and `--after-source` pointing to the
source text files, and `--output` pointing to a new JSON path. The verifier
checks exact case coverage, replay parity, zero allocation observations and the
measured native storage delta. Parent acceptance remains open.
