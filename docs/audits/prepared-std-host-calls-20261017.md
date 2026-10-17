# Prepared std Host Call identity lookup

Development slice of #5395, 17 October 2026.

The installed std runner previously searched every lowered Host Call for each
request and cancellation, then searched the retained request vector to record
a first observation. Preparation now builds a fixed 128-entry index for the
existing 32-node, four-calls-per-node profile. Play borrows the exact original
LoweredHostCall through the index; no contract, authority or effect is rebuilt.
Both dimensions are checked before flattening, so an out-of-range call cannot
alias the next node. Admission and missing-identity refusals remain explicit.

First-only recording retains the first RequestId and input in original order.
Math and Face construction retain every completion exactly as before; their
append path also updates first-observation state. The original vector is handed
to post-Play report translation unchanged. The fixed arrays use 256 bytes of
indices and 128 bytes of flags; on this native host the wrapper is 424 bytes,
400 more than the former Vec alone. Request capacity is reserved before Play
for at least all unique lowered calls. Existing append-all vector behavior is
retained; this does not establish whole-Host allocation-free execution.

The differential test permutes all 128 identities and checks 4,096 mixed
first-only and append-all records against the original linear oracles, including
pointer identity, ordering, exact requests, unchanged capacity and zero tracked
allocations. Negative tests cover empty/missing keys, each dimension outside
the profile, duplicate first-match behavior and oversized preparation.
Focused native tests also exercise scalar tape, ordinary math, layout and Face
composition, canonical allocation contracts and authored cancellation.

Before capture: clean Source 2313ac8a4d6be87474c186d0788ba09676bba2b5,
18 release conformance tests, 89 artifact hashes, 190 rows and 123 repeated
distributions independently verified. The intervening commits through the
starting dev head 130fe4a34 change only browser JavaScript/tests and its audit;
the native implementation is identical. After measurements and matched output
custody are retained separately below. Uncontrolled one-process before/after
observations cannot establish a calibrated speedup or regression gate.

No PR or CI requested, per operator instruction. Parent #5395 remains open
until its full measurement matrix and closure evidence are present.

## Verified before/after observations

After Source: `3216580cf9e3846617c0b3b4f98da63c0691401b`. Both supported hosted captures passed
all 18 release conformance tests. Each independently verifies 89 hashes, 190
raw records, 123 repeated distributions and 27 installed phase partitions.
The comparison checks all 27 matching rows for exact authored Source digest,
useful output count/digest, kernel decisions/invocations, value capacities and
complete retained Sign tuples. Boot/Play/Plan identities belong to their own
source-pinned run; no cross-source Plan identity is asserted.

| Case | Before Play mean, ms | After Play mean, ms | After/before |
| --- | ---: | ---: | ---: |
| pure-chain-1-fused | 0.370 | 0.384 | 1.037 |
| pure-chain-1-local-unfused | 0.340 | 0.345 | 1.015 |
| pure-chain-32-fused | 2.371 | 2.257 | 0.952 |
| pure-chain-32-local-unfused | 2.988 | 2.472 | 0.827 |
| pure-chain-8-fused | 1.151 | 1.280 | 1.111 |
| pure-chain-8-local-unfused | 1.313 | 1.238 | 0.943 |
| hosted-pipeline-1-gears-32-adds | 0.385 | 0.329 | 0.855 |
| hosted-pipeline-32-gears-32-adds | 5.745 | 5.058 | 0.880 |
| hosted-pipeline-8-gears-32-adds | 1.420 | 1.344 | 0.946 |

This includes the unfavorable 8-Gear fused observation. These are descriptive
three-batch means from one uncontrolled process per variant. The report retains
every raw sample and sample SD, full-call and preparation metrics alongside
Play. No causal speedup, independent-process confidence interval, universal
regression threshold or parent completion follows. The indexing removes
linear lookup work by construction; elapsed-time uncertainty remains distinct.

Reproduce both captures from their measured clean commits using:

```sh
cargo +stable xtask prove runtime-performance --output <new-directory> --values 1000000 --repetitions 3 --decisions 1000 --samples 1000 --hosted-pipeline true
```

The raw archives, capture verifications, matching script, comparison samples,
native test log and checksum manifest share the prefix
`evidence/std-host-call-index-20261017-`. Run the existing capture verifier on
each extracted directory before running the retained comparison script with
`before-directory after-directory output.json`. Strict std library Clippy,
workspace formatting and diff whitespace checks pass.

A local perf probe refused hardware events with `perf_event_paranoid=4`;
its log is retained. Hardware instruction/branch counters are unavailable,
not zero. No system security setting was changed.
