# Typed finite periodic oscillator

For #5676, Audio owns the portable Q8 phase and integer cubic-flow laws formerly
inside compact voice. Speech calls those same laws; voice policy and mixing stay
with Speech. The oscillator has no voice parser, timer or device effect.

The request retains the existing typed sample-rate basis, Frequency control,
Phase reference (including authored whole turns, anchor and exact time offset),
relative amplitude and an explicit unscaled cubic-flow reference. The first
sample uses the supplied phase at its retained reference time; emitted frames
advance from there at the supplied sample rate. This is not a physical pressure,
full-scale PCM or continuous exact-real oscillator claim.

The selected realization uses the existing Q8 period envelope 2048..131072
(8..512 frames), sample rates 8000..192000 and at most 65536 frames per request.
It includes every original 58..69-frame voice period and fractional Q8 periods.
Exact wide quotient/remainder projections retain the original rational request.
The recurrence uses the projected Q8 period and Q15 amplitude; nonzero remainders
record quantization rather than claiming the realized frequency/amplitude equals
the original fraction. Amplitude ratios above two and unsupported frequencies
refuse explicitly. Unsupported periods refuse before phase multiplication.

Strong Native snapshots retain request, projection, profile, phase and budget
laws. Pure Source produces explicitly named candidate carriers and checks the
same authored laws before recurrence. Build-time generation prefixes and stages
the declared law expressions; it does not copy oscillator mathematics into a
second implementation. The finite step has 23 ordinary expression Gears, including
18 law checks. Staging respects the existing portable decoder depth limit; Core
construction eligibility and decoder limits are unchanged.

Independent frame-coordinate references cover integral/fractional periods,
negative phase, amplitude scaling, whole cycles and reset. Canonical cursor reloads
exercise partition-independent state. Candidate forgery, bounded budget, refusal,
exhaustion and cancellation are separate outcomes. Prepared expression evaluation
is checked for zero allocations/reallocations and unchanged output capacity.
The separate installed-host test uses ordinary Source expansion, planning and
Play, fresh-host partition resume and finite queues. Each Item invokes one finite
ordinary Play; partitions group caller invocations and carry only the checked
cursor. It does not claim a hidden multi-frame runner. Source-pinned final results
are recorded separately in the evidence file.

Validation at `d857468de1a0cb501e055b4a7b66a3bddf03705c`, based on accepted `9780308650e454a091edaac55a05f38f97ed77e9`:
six oscillator contract/reference tests, one ordinary Plan/Play test, one shared
flow test and all 60 default Speech library tests pass. This includes legacy and
resolved-cycle speech parity. Strict focused Audio/std Clippy and pinned workspace
formatting pass. The evidence file records exact tested files and raw log hashes.

The candidate is refreshed onto accepted `560916ec4f361901e23449f7b44d984dc067c1c3`.
All 13 tested files remain byte-identical to the measured source above. The
standalone Speech footprint lock now records the Audio build dependency and its
Data/Time dependencies without registry version changes. All eight exact Cargo
lock checks used by the repository preflight pass at
`b6980c2666715987841abbff59d79e411a1a4e88`. The local xtask build hit a full
filesystem before reaching those checks; after reclaiming inactive compiler
metadata, the identical lock-check commands were run directly.

Parent #5217 and composed-synth sibling #5678 remain separate acceptance gates.
