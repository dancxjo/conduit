# Measured ordinary reluctant replacement (#5568)

The hosted pure-Value profile now completes the measured producer → cost facts →
persistent decision → ordinary new Plan/Play → acknowledged output chain.
This closes the missing measured connection described in the earlier
[execution-bridge audit](reluctant-execution-bridge-20261020.md), whose partial
status describes its historical snapshot. Publication remains tracked in #6078.

## Source and finite protocol

The only registered release capture used clean, unchanged source
`70e89ed3244602c65fee21f3052e2281012d037b`. The supported command was
`cargo xtask prove runtime-performance --reluctant-replan --output PATH`.
The [registration](../performance/reluctant-replan-registration.md) fixes three
paired observations, minimum 100 μs persistence, two-second maximum gap,
ten-second window, one-second cooldown, one-millisecond conservative minimum net
benefit and 1,024 future requests before any timing outcome. There was one
release family, with no retry or selected successful subset.

An actual checked baseline-32 expression receives U64 7 and returns 231. One
Std Host offers the existing generic Host Call and direct-local expression Backs;
their sealed Plans differ while Source/checked/expanded work and typed Fores
agree. No new provider, kernel or authority grant is introduced. Each explicitly
submitted request has ordinary admission, kernel execution, terminal Sign and
acknowledged output. Four separately admitted pure requests create the middle
candidate queue burst. Persistent pairs alternate execution order. These are
issuer-owned finite queued requests, not an assertion about OS scheduling.

CLOCK_MONOTONIC on Linux x86_64 (Intel i7-12700) supplies identified measured
intervals. Resolution is endpoint quantization, not a certified accuracy bound,
confidence interval, WCET, deadline guarantee or future speedup guarantee. The
raw worker status retains toolchain and physical boot. Logical Host/Boot and
actual terminal Sign identities bind every forecast. These are identified local
producer observations, not externally authenticated or cryptographically signed
measurements.

## Observed result

All five replacement stages were explicitly admitted. Their conservative upper
costs were preparation 6,928.817 μs; transfer 0.197 μs; residency 0.204 μs;
migration 0.433 μs; continuity 21.972 μs: total **6.951623 ms**. Preparation
conservatively includes initial common Source/Host/reference Plan preparation.
Transfer prepares the retained eight-byte input. Residency and migration are
measured checks of this profile's actual empty external resources, pure state
and completed old Play; they do not model a device or live mutable migration.
Continuity projects and fingerprints the retained old receipt.

For each pair the policy calculates
`(old lower response - candidate upper response) * 1024 - overhead upper`.
The noisy window's net values were **88.778041, -1060.894407, 53.108025 ms**,
so it retained the old Plan with `NetBenefitNotPersistent` and no execution.
Persistent net values were **236.667193, 44.780857, 37.056825 ms**, all strictly
above the one-millisecond threshold. The real production bridge therefore
submitted the first future request under the candidate's distinct Plan through
ordinary current admission. Considering the same old recommendation again
returned `AlreadyRecommended`, with no new output or Play. All 1,024 future
requests completed. This supports this finite decision; it does not claim a
statistical universal gain or prove that realized future timing equals a forecast.

There were **1,041 distinct consecutive Plays**: initial old request; ten noisy
requests (six paired plus four backlog); six persistent requests; 1,024 horizon
requests. Every output is the correct 231. Mandatory kernel completion/delivery,
terminal identity and unchanged Value capacity are retained. The old completed
receipt SHA-256 remains
`830d34bd422818bb436fcf2b02e7660b9bc3768515a2168b1acee6b35cbc9f50`.
No in-flight Plan is changed and no effectful work is replayed.

Response intervals include ordinary per-call preparation/validation, output
acceptance, evidence projection and raw report disposal. File I/O is outside the
interval. This is not full-process startup/RSS accounting, Body/UI policy,
resource-bearing or stateful replacement, nor a stable-release claim.

## Independent checks and retained evidence

The [complete capture](evidence/reluctant-measurement-20261020/family-1/status.json)
retains compressed raw JSONL, worker/command status and complete stderr/stdout.
The capture's manifest hashes logical decompressed files; the outer
[manifest](evidence/reluctant-measurement-20261020/manifest.json) hashes archived
bytes. The independent Python checker verifies exact rational arithmetic,
clock intervals, five cost sources, forecast-terminal custody, identical logical
work, actual output/terminal/kernel records, contiguous unique Plays, finite
backlog, no-replan rationale and old receipt continuity. It does not independently
reimplement the complete Rust Plan fingerprint law; ordinary worker admission
and `verify_plan` enforce that at the pinned source.

[Verification](evidence/reluctant-measurement-20261020/verification.log) passes
against the archived capture. All [12 semantic corruption controls](evidence/reluctant-measurement-20261020/negative-controls.log)
reject: missing old receipt/stage/horizon/backlog, fabricated net/clock interval,
forecast-terminal substitution, wrong output, replayed Play, noise retrigger,
erased continuity and foreign cost custodian. These deliberately bypass the
outer file manifest to exercise causal checks independently of hashing.

Before freezing source, all seven bridge tests, strict worker/full-xtask Clippy
and focused formatting passed. Accepted dev integration at
`743998e9d96997a4cad93df577d36cd25545d9e6` changed only browser CI configuration
and its documentation; all seven bridge tests pass there as well. That later
integration is not relabeled as the measurement source. The earlier debug/dirty
exploratory probes and disk-exhausted development check remain excluded in the
historical audit/local diagnostic logs, not accepted performance families.
