# Hosted reluctant-replacement observation protocol (#5568)

Entrance: `cargo xtask prove runtime-performance --reluctant-replan --output PATH`.
PATH must be new and outside tracked source or under `target/`. The release
worker requires Linux, committed clean source, and unchanged source afterward.
Raw JSONL is flushed incrementally; outer and worker status retain failures.
A completed capture is not automatically profitable or accepted.

The fixed workload is the checked `baseline-32` Value expression: 32 exact U64
additions, input 7 and output 231. One Std Host offers the genuine generic
Host Call and direct-local Backs. Source/checked/expanded meaning, typed Fores
and request bytes agree; their ordinary sealed Plans differ. No new Back,
physical lane, external device or authority grant is introduced.

Before any timing outcome, the protocol declares three paired observations per
window, a 100 microsecond persistence span, two-second maximum gap, ten-second
window, one-second cooldown, one-millisecond minimum conservative net saving,
and 1,024 explicitly submitted future requests. The noisy window alternates
idle, four already queued explicit pure requests, and idle before the candidate.
The steady window rotates previous/candidate call order. Every queued/calibration
operation has its own ordinary Play and retained terminal/output evidence.
The workload executes on the reference Plan if replacement is not profitable;
it is not invented only after a positive result.

Each response includes ordinary per-call kernel preparation, validation, output
acceptance, report projection and raw report disposal. The eight-byte input is
prepared once. Queue waiting and service have separate actual clock intervals;
additional setup/transfer outside those windows are explicitly zero. Artifact
file I/O is outside these response windows. This is not complete-process/RSS
accounting or a future timing guarantee.

All five additional replacement stages are measured: complete initial Source,
Host and Plan preparation (conservatively including common reference setup),
input preparation/transfer, the explicit no-external-resource residence check,
the completed/pure/no-mutable-state handoff check, and old-receipt continuity
projection/hash. Non-applicability is measured and labeled, never a missing fact
silently treated as zero. Actual resources or live mutable migration remain
unsupported by this profile.

The same declared horizon and threshold apply regardless of result. The planner
uses `(old lower - candidate upper) * horizon - upper sequential overhead` and
requires every persistent observation to exceed the threshold. Ordinary current
Host admission remains required after recommendation. A repeat consideration of
the same old Plan must not retrigger. Full old/new Plan/Play/Sign/output custody,
exact costs and the old-receipt fingerprint are retained.

CLOCK_MONOTONIC resolution supplies endpoint quantization only. The capture does
not certify clock accuracy, statistical confidence, WCET or universal speedup.
An unprofitable or contaminated/failed family remains evidence and cannot close
the positive-benefit acceptance criterion. Keep exploratory dirty/debug probes
separate. Independently check the captured directory with
`proof/performance/verify-reluctant-replan.py` and the companion semantic
corruption controls. Stable publication and broader Body/UI policy remain separate.
