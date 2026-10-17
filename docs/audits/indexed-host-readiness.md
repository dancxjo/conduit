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
