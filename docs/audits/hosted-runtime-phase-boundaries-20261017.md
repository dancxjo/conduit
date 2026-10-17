# Hosted runtime preparation and Play wall boundaries

Local #5395 implementation slice, 17 October 2026. The ordinary std Host already
exposes optional `PreparationStarted`, `PlayStarted`, and `PlayFinished` markers.
The performance harness now observes those existing boundaries for each ordinary,
local and fused 1/8/32-Gear pipeline, using three fixed timestamp slots.

Each successful call retains marker offsets, observed call wall time and four
segments: before installed preparation, Back/kernel preparation, Play, and
after Play. Segments partition the observed call exactly. The outer existing
full-call measurement includes observation overhead. Markers are native wall
observations, not authored semantic time or process CPU samples. Preparation
includes all installed setup between its two boundaries; Play includes Host
coordination and required evidence; post-Play includes report translation and
other work until return. This does not isolate arithmetic, copy, individual
Sign or teardown cost. Existing cold parse/check/expand/selection/seal rows and
separate Host/harness teardown measurements are retained.

The observer rejects missing, duplicated, reordered and extra markers. It adds
no allocation or new marker to production execution; the optional observer
adds five wall-clock samples per measured call. Repeated distributions retain
the five durations as separate metrics. The capture verifier derives the
partition from raw marker offsets, enforces nonnegative monotonic bounds within
the outer call and raw/report/distribution agreement. Legacy captures without
phase observations remain distinguishable and readable; mixed partially
observed pipeline captures refuse.

Use the supported `cargo +stable xtask prove runtime-performance` entrance with
`--hosted-pipeline true` to retain these observations from clean committed
source. Performance remains report-only. Full streaming, DSP, browser, instruction
and copy attribution and stable parent acceptance remain open.

## Retained release capture

Measured clean source `06fe8d914d2798ce90de7010f6dd25609d01e05f` using
`cargo +stable xtask prove runtime-performance --output NEW_OUTSIDE_DIRECTORY --values 1000000 --repetitions 3 --decisions 1000 --samples 1000 --hosted-pipeline true`.
All 18 release conformance tests passed before measurement. The original
worktree stayed clean through completion. This is one process, with three
within-process batches, uncontrolled host load and no affinity/frequency change.
Compiler/model/OS/profile and raw metrics are manifest-bound in the capture.

Verification covers 89 artifact hashes, 190 raw rows, 123 repeated distributions
and all 27 phase partitions, alongside the existing exact output/Sign/custody
checks. The earlier uninstrumented capture still verifies with zero observed
phase partitions; its data are not silently upgraded.

Ordinary 64-output calls, milliseconds (mean ± sample SD across three batches):

| Gears | Before preparation | Back preparation | Play | After Play | Observed call |
|---:|---:|---:|---:|---:|---:|
| 1 | 0.050 ± 0.015 | 0.373 ± 0.034 | 0.361 ± 0.059 | 0.049 ± 0.005 | 0.833 ± 0.076 |
| 8 | 0.080 ± 0.017 | 0.509 ± 0.082 | 1.415 ± 0.111 | 0.065 ± 0.011 | 2.069 ± 0.220 |
| 32 | 0.132 ± 0.008 | 0.873 ± 0.100 | 5.095 ± 0.014 | 0.114 ± 0.004 | 6.214 ± 0.124 |

In this finite fixture, the ordinary 32-Gear call spends about 82% of its
observed wall time inside Play. Preparation alone does not explain its scaling
cost. Play still includes expression evaluation, dispatch, transport, Host
coordination and required Signs; these observations cannot apportion those
components or predict million-item steady throughput.

The separate local/fused profiles are retained too. At 32 Gears their mean
Play windows are 2.481/2.204 ms, while preparation is 0.641/0.823 ms; their
full observed calls are 3.339/3.241 ms. Reduced Play work can be offset by
preparation, and this single-process observation supplies no stable speedup
claim or comparison against historical different-source captures.

`check-runtime-phase-boundaries.py --capture CAPTURE --output NEW_RECEIPT`
first verifies the original, then rehashes temporary copies with an inconsistent
partition, nonmonotonic markers, and an otherwise consistent phase call exceeding
the outer measured call. All three refuse for their intended reason without
writing a success receipt. Originals remain untouched.

Evidence: `evidence/hosted-runtime-phases-20261017.json`,
`evidence/hosted-runtime-phases-negative-20261017.json`, and
`evidence/hosted-runtime-phases-20261017-raw.tar.gz`. The raw archive retains
the complete manifest-bound capture and conformance logs. No CI requested.
