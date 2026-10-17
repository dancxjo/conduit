# Reinsert earlier cancellations while later work remains queued

Local #5231 coverage for the cancellation search bound introduced by the large
Host-call stress slice. The previous batch test rearmed a whole completed batch;
these vectors reinsert an earlier slot while a higher slot remains eligible.

A small fixed fixture uses ordinary request dispatch, remote control input,
cancellation, completion and resume APIs for every transition. It dispatches slot
zero's first cancellation, consumes that completion and rearms request one in
slot zero, then cancels it while slot one's request-zero cancellation is still
queued. No ready flags, pending records or search bounds are seeded. Read-only
assertions inspect final indices, values and driver state.

| Race before the next poll | Observed cancellation order (node/request) |
|---|---|
| Neither call completes | 0/0, 0/1, 1/0, 1/1 |
| Fresh slot-zero request completes | 0/0, 1/0, 1/1 |
| Older slot-one request completes | 0/0, 0/1, 1/1 |
| Whole run cancels after reinsertion | 0/0, then no further dispatch |

All ordinary flows finish both Backs, reclaim every value and pending index,
and reach Drained. Global cancellation invokes driver cancellation and leaves
no request/cancellation dispatch, retained obligation, value, or ready work.
Each ordinary flow retains 32 Signs without a retention gap. Packed and linear
selectors produce matching normalized rows; this is not a full Sign-stream
parity claim. In the raw schema, settled=-1 means neither completion wins, poll
codes are 10*node+request, and u32::MAX marks an unused fixed observation slot.

## Test sensitivity

Two deliberate temporary mutations checked that the new vectors exercise the
critical transitions. Omitting the rewind made the older slot-one cancellation
appear before fresh slot-zero request one. Omitting completion-first accounting
left one undispatched cancellation after the completion race. Both mutations
caused the expected test assertion failure, not compilation failure. Production
source was restored byte-for-byte and has no committed change in this slice.
These are sensitivity checks, not claims of newly discovered production bugs.

Raw positive and mutation logs, full-suite logs and Clippy output are retained
in `evidence/cancellation-interleaving-raw.tar.gz`. The local receipt in
`evidence/cancellation-interleaving.json` records eight artifact hashes, source
content hashes and normalized outcomes. It is unit-conformance evidence rather
than a clean-source release capture. Reproduce positive vectors with:

```sh
cargo +stable test -p conduit-kernel --lib host_many::interleaving -- --nocapture
cargo +stable test -p conduit-kernel --features linear-ready-reference-test \
  --lib host_many::interleaving -- --nocapture
```

Local validation: both focused vectors pass with both selectors; alloc and
linear-ready-reference kernel suites each passed 212 tests. Formatting, strict
alloc all-target Clippy and diff whitespace passed. No wall-clock, browser,
physical-device or parent-issue completion claim is made. No CI was dispatched
or awaited.
