# Fixed packed ready selection

For #5561, the fixed kernel retains one readiness state with packed bits and
cursor-ordered traversal. Publish, Cord vacancy, Host Call completion, closure,
remote fan-out and retirement update those same bits. Ready candidates retain
existing completed/pending-Host eligibility checks; no indexed Host Call table
or new scheduling policy is claimed. `has_ready_work` also skips sleeping nodes.

The private module uses the previous N-byte readiness storage ceiling. Only
ceil(N/8) bytes encode membership; unused bytes retain the existing generic
capacity layout without unstable generic-const arithmetic. Repeated wakeups set
one bit, so cannot duplicate membership. The iterator visits each candidate at
most once in the same cursor order as the old linear scan. Selection is bounded
by packed bytes plus ready candidates and their existing pending-call scans.
There is no allocation, new queue, priority, SMP or preemption facility.

The retained native release comparison has eight samples per 1/8/32 active-node
loaded/sparse shape, at clean Sources:

- Reference: `c7e07304be54a2f1241d5ce916cf3a4a4817ead1`.
- Packed readiness: `35f59a5db913ed2f71e79aece2c2ebe2ce60ee4a`.

All 96 raw rows, log digests and variation summaries are retained in
`evidence/sparse-ready-release-comparison.json`. Each paired row preserves steps,
Sign records/bytes, context checks, ports, Cord scans, queue peeks and the real
capacity-one pressure refusal. At 32 active nodes with one ready input, the old
selection scans 96 nodes/pending slots; the packed path checks two candidates,
12 packed bytes and two pending slots, for the same two steps. At eight nodes,
sparse counts change from 24 nodes/pending slots to two candidates, three bytes
and two pending slots. Cord work is unchanged and remains visible.

Observed sparse-32 elapsed-wall medians are 2667 ns reference and 487.5 ns packed;
loaded-32 medians are 52940 ns and 13279.5 ns. These instrumented samples ran in
separate unpinned native processes under concurrent development activity, with
first-use effects and no calibrated CPU-frequency control. They are not CPU-time
measurements, production latency thresholds, confidence in a universal speedup,
or physical-device evidence. The deterministic scan reduction is the primary
result. Small/loaded shapes and full variance remain in the raw report.

Validation passed the allocation-enabled kernel suite (78 unit tests plus all
integration suites), strict all-target Clippy with warnings denied, and the
no-default-feature kernel check. The packed iterator is compared to linear
round-robin across active counts/cursors and finite membership patterns spanning
byte boundaries. A separate allocator probe runs 32 yielding nodes through 160
actual scheduler steps to drained completion: order stays fair and allocation
and reallocation counts are both zero. Existing fuel, cancellation, pressure,
remote input/output, Host Call, terminal and Sign conformance remains covered.

The ignored `scan_characterization` release test is a repository-internal
measurement fixture, using the existing actual FixedScheduler; it is not a
replacement product scheduler or authored Source/Plan/Play benchmark. #5395
retains its broader product and overhead attribution gates. #5231 retains its
indexed pending-call and broader realtime/FARGAN requirements.
