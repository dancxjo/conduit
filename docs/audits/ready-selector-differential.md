# Fixed-kernel selector differential

For #5562, a named repository diagnostic feature replays legacy rotating linear
candidate enumeration inside the actual FixedScheduler. It shares the current
packed readiness state, eligibility checks, Step, value storage, cancellation,
Host Call and Sign machinery. Default product builds keep packed selection; no
global planner/shared-pool policy or second scheduler is introduced. This is
not a byte-for-byte restoration of the former Boolean-array storage; #5561
retains its separate production old-bool and packed baseline captures.

At clean Source 6aabdd2f4bd95a74a072432becf7a203863e05e7, both modes pass the same 80 unit tests and integration
suites: 141 matched named tests cover supported fuel, Host Call, completion,
abnormal/recovery, cancellation, remote input/output, pressure and value
contracts. Both strict all-target Clippy modes deny warnings. The independent
membership oracle remains active and checks packed flags against Boolean
expectations rather than deriving expectations from packed storage.

Release replay captures eight trials for loaded/sparse capacities 1/8/32/256.
All 64 paired normal/remote Sign traces, consumed-byte counts, readiness,
remaining value items/bytes and terminal states match exactly. Capacity-one
pressure refuses sequence 1 without advancing sequence custody. Both modes
drain consumed storage and become Idle. Repository evidence deduplicates the
identical snapshots per shape while retaining every timing row; complete raw
captures and log digests remain in task outputs.

One-node sparse wall medians are 1545 ns diagnostic linear versus 1756.5 ns
packed: packing loses here, with wide variance. Sparse 256-node medians are
3442 versus 2372 ns, while candidate/pending scans drop from 768 to two plus
96 packed bytes. These unpinned instrumented samples under concurrent activity
are report-only, not calibrated speedup or realtime guarantees.

Storage is equally explicit: the diagnostic fixture admits 4096 normal/remote
Sign slots, making whole-kernel sizes 181224 bytes at one node and 325552 at
256. Readiness still reserves N bytes in either mode. An explicit bounded
8 MiB fixture thread stack accommodates large native construction; it is not
a physical worker lane or tiny-device RAM proof. Native test execution and
timing do not establish #5232 physical parallelism.
