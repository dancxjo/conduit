# Durable owner execution custody

The remaining normal-owner boundary of #6102 and #5571 is implemented on
source `f8aaabeb829469016a52341a5358eae358133ff9`. This completes the scoped
implementation alongside the earlier [API lifecycle proof](plan-replacement-custody-20261020.md)
and [incremental cold oracle](incremental-host-line-20261020.md).

The foreground owner previously replaced its sole durable execution receipt.
It now retains immutable, content-addressed receipts and an ordered validated
index through the existing recoverable owner transaction. Metadata publication
cannot erase prior receipts. A terminal result enters in-memory custody before
storage publication; a failed write fences subsequent preparation until that
exact result is retained. Retrying publication never runs the kernel again.
The normal owner exposes historical receipts through `ExecutionHistory`.

## Concrete evidence

The supported Todo owner executes four real actions and reads under ordinary
planning and the production kernel/Host. Its first verified receipt survives
later executions and recovery unchanged. The old and latest receipts have
distinct Plan, Play, terminal Sign and output fingerprints under the same Body.
The existing continuity test also recovers the Body on a separately admitted
Host without a cache. History remains scoped to its original installed-owner
root; it is not automatically transported to another custodian.

A real worker finishes after its transaction path becomes unwritable. The owner
retains its exact terminal receipt and pending-publication flag, drops its live
action handles, and later publishes once after storage is restored. Repeated
publication and recovery retain exactly one receipt and unchanged checkpoint
file hashes. A separate capacity test fills the declared profile with explicitly
identified metadata fixtures, then proves normal owner preparation refuses
before Host transfer or checkpoint effects. Those fixtures are not presented as
64 executed Plays.

State tests cover interrupted journal replay, missing indexes, corrupted records,
cross-Body/stale transactions, oversized or inconsistent terminal receipts,
legacy migration, metadata repetition and refusal without eviction. The earlier
three API cases retain cancellation/drain, uncertain external acknowledgement,
provider loss and an independent resident Play. Unknown delivery stays unknown;
no receipt authorizes replay.

## Finite profile and recovery

Each installed-owner root admits at most 64 immutable receipts, each at most
128 KiB serialized, with at most 1,024 additional record-framing bytes. Index
reads are capped at 192 KiB. Directory inspection admits at most 64 + 2 slots
for final/staged files; only a retained journal may complete its own unindexed
slots. The existing owner journal remains capped at 2 MiB. Preparation adds at
most two records (known legacy latest and current latest). Receipt parsing and
JSON response construction allocate bounded transient objects; serialized byte
limits are not claims about allocator overhead or Play worker residency.

There is no eviction or unbounded history. Full capacity refuses before a new
Play. Index/record digests bind custodian Host, Body and exact receipt. A legacy
latest-only index migrates its known latest and explicitly records an unavailable
historical prefix; it cannot invent lost earlier outcomes. Reads mark custody as
historical-only and coverage as `this-installed-owner-root`. Historical custody
is neither execution authority nor evidence that a Play remains live.

A process crash before any terminal evidence reaches durable storage cannot
fabricate that evidence. The persisted started claim remains uncertain. This
work proves the concrete owner transaction/retry boundary and does not claim
universal cross-Host archives, remote-network delivery or physical reboot proof.

## Exact-source validation

The worktree was clean at source `f8aaabeb829469016a52341a5358eae358133ff9`
before and after validation. `cargo xtask check todo-durability` passed 58 tests;
one separately captured retained-DOM test remains explicitly ignored. Six shared
Parsec/cancellation/release regressions passed. Strict product Clippy and focused
formatting passed. Independent verifiers check all three API and three owner
records, immutable receipt digests, exact replacement identities and no-replay
custody. Seven API and six owner corruptions were rejected, including a rehashed
old-Play substitution. This is correctness evidence, not a speedup measurement.

[Validation receipt](evidence/owner-execution-custody-20261020/validation.json)
and [hashed raw-log manifest](evidence/owner-execution-custody-20261020/manifest.json)
retain commands, source, counts and compressed complete records. Run the two
`verify-*-custody.py` and two `check-*-custody-negatives.py` scripts in
`proof/performance/` against the retained `todo-durability.log.gz`.
Implementation closure does not assert Integration/main publication or a release;
that follow-through remains with #6078.
