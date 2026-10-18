# Bounded local dispatch with the full FARGAN graph

For #5231, PR5960 supplies the previously missing checked full-float FARGAN
frame. Ordinary Plan/Play runs its conditioner and four recurrent subframes:
768 nodes, 1,211 Cords and 160 PCM samples, with explicit conditioner history,
previous period and recurrent state. Numeric backs are general public owners;
layer ordering remains Source. The pinned upstream C implementation is an
oracle only. [Graph/profile provenance](../../proof/fargan/README.md) retains
the recovery boundary and private-parameter instructions.

The scheduler follow-up normalizes ready publication against canonical
completion and the existing per-node Host Call index. Queued Cord inputs,
space and terminal notifications remain canonical while a call blocks its
Back; consuming its correlated completion rearms execution. Repeated wakes
remain idempotent and the cursor-ordered fairness policy is unchanged.

A completion count fits in the unused tail of the existing N-byte dispatch
reservation once N is at least twice the native word width in bytes. The
original Boolean completion flags remain canonical. Normal/abnormal completion
increments only on the first transition; readiness clearing preserves this
count. Tiny capacities retain the original early-exit check, bounded to at
most 15 flags on supported targets. No field or storage reservation is added.
Tests cover disjoint ready/summary/count storage, duplicate retirement, active
prefixes, refused mandatory Signs, boxed preparation and all 65,536 node
identities. This is one readiness policy, not a silently selected priority or
alternative runtime implementation.

## Source-matched real-graph measurement

The current comparison uses development after the platform repair and FARGAN
admission: before `a73aacf4da5969144e2eb86f5d75100f172c9f73`, after
`2bd626ddd13487c9977830151a8d9b8a9c15e594`. Both use the same existing boxed
Std preparation API, graph, numeric/expression sources, precision, model,
Plan and measurement boundaries. They were compiled at the same filesystem
path with the pinned toolchain. The separate development refresh matters:
its equivalent Sign-budget predicate rewrite is inside measured work, so
older timing results are not substituted for this comparison.

The supported entrance is `cargo xtask prove runtime-performance` with the
explicit private FARGAN fixture and output options. It shares #5395's timers,
process CPU clock, allocator and artifact machinery. Three processes per side
each run two 24-frame repetitions. The first pair is captured through xtask;
two additional pairs replay its exact prebuilt workers in alternating order
on CPU 1 without recompilation. State accumulates within each trajectory and
resets between repetitions. All 144 pairs preserve exact Plan/output/Sign
hashes and work counts: 3,165 Steps and 6,976 Signs per frame, structural drain,
zero pending Host Calls and zero Play allocations/reallocations. The fixed
scheduler object is 2,642,384 bytes on both sides; model, stores, owners and
observer storage are additional preparation allocations.

| Observed phase | Before ms/frame | After ms/frame | After/before |
|---|---:|---:|---:|
| Whole Play wall | 22.257 | 21.818 | 0.980 |
| Whole Play process CPU | 22.257 | 21.817 | 0.980 |
| Numeric Back wall | 4.389 | 4.334 | 0.987 |
| Expression Host Call wall | 13.369 | 13.110 | 0.981 |
| Step remainder wall | 4.425 | 4.301 | 0.972 |
| Fresh owner preparation wall | 2835.526 | 2764.244 | 0.975 |

Whole Play ratios are 0.987, 0.983 and 0.971 across process pairs; the aggregate
ratio is 0.980 with descriptive paired-resampling interval 0.971–0.987. This
shared-host result is a modest approximately 2% Play improvement, not a
universal performance or regression bound. Numeric and expression timings
also vary; their changes are not credited as ready-selection speedups.
Hardware cycle/instruction counters were unavailable.

The Step remainder includes kernel, queue, mandatory Sign, Back commit,
observation and fixture source/sink work. It is not isolated selector CPU.
Dividing it by the fixed Step count gives approximately 1.40 versus 1.36 µs
per observed Step. Whole Play excludes caller input encoding, Plan cloning and
post-Play decoding; these are fresh Value invocations, not a reusable streaming
owner or a physical audio/application latency measurement.

First Source checking averages 147.4/147.3 ms before/after; first model admission 172.7/173.6 ms; check/expand/Plan 819.4/807.4 ms per repetition. OS page-cache coldness is not claimed.

Fresh owner preparation remains about 2.8 seconds per frame invocation and
dominates that usage. Within Play, prepared expression Host Calls remain the
largest measured component, about 60% of wall time; numeric work is about 20%.
Expression preparation/traversal and physical fusion retain their separate
#5396/#5397 ownership; streaming remains separate speech work. No real-time,
perceptual-quality, native-preemption or parallel-execution claim is made.

The full-frame oracle comparison feeds the graph its own accumulated history
and recurrent state. Maximum observed PCM error is about 8.8e-6 over the pinned
24-frame trajectory. Finite-only numeric codecs and output/state decoding
reject nonfinite values. These finite empirical guards are not a global
precision theorem. Parameters and oracle binaries are not published.

## Small, loaded and sparse selection

The current source also passes nine alternating process pairs of the existing
1/8/32/256 loaded/sparse fixture: 1,152 samples, 67 artifact hashes and exact
Sign, output, readiness, value, terminal, allocation, copy and size parity.
Its diagnostic linear enumeration shares packed storage; it is not the old
Boolean footprint or a second production policy. Ratios below are packed over
linear, with descriptive process-pair resampling intervals.

| Nodes | Case | Wall ratio | 95% interval |
|---:|---|---:|---:|
| 1 | loaded | 0.932 | 0.757–1.189 |
| 1 | sparse | 0.963 | 0.788–1.118 |
| 8 | loaded | 1.069 | 0.988–1.148 |
| 8 | sparse | 0.897 | 0.835–0.973 |
| 32 | loaded | 0.999 | 0.959–1.049 |
| 32 | sparse | 0.701 | 0.652–0.763 |
| 256 | loaded | 1.039 | 0.983–1.117 |
| 256 | sparse | 0.252 | 0.237–0.271 |

Sparse 32/256 cases are faster in all nine pairs. Tiny/loaded distributions
remain mixed: loaded eight-node selection has an unfavorable point estimate,
with an interval crossing one. This does not establish universal tiny-Host
timing equivalence or a calibrated regression gate. No unadmitted selector
switch is introduced. The tiny completion check deliberately retains its
original work. Existing [two-ready/10,000 execution](large-sparse-release-20261017.md)
retains four candidates plus 480 byte probes versus 30,000 linear candidates;
current full kernel suites retain the associated sparse/fairness contracts.

The current completion-prefix replay retains 96 exact full Sign pairs and 24
artifact hashes against its historical source-pinned baseline. Native probes
remain 17/136 for capacities 1/8 and fall 544/4,352 to zero for 32/256. All four
instrumented object sizes remain unchanged. This internal fixture has no
useful output; historical unpinned process timings are descriptive and are
not substituted for the current whole-graph result. The Rust timing entrance
is unchanged; location/budget-validation wrappers are independently hashed.

## Rejected candidates and remaining bounds

The original extra-usize completion-count candidate regressed whole FARGAN
Play about 8% initially and about 9.6% over three fresh process pairs. Boxing
both sides still regressed about 6.5%, despite a smaller Step remainder.
Isolating the scalar count regressed about 4.7%; wake normalization alone
measured about 2.4% faster. Packing flags and a count into the completion
reservation also regressed about 4.8%. Those alternatives were not selected.
Their exact raw results and source identities remain in the
[inline receipt](evidence/fargan-inline-regression-20261017.json),
[inline archive](evidence/fargan-inline-regression-20261017-raw.tar.gz),
[variant receipt](evidence/fargan-scheduler-variants-20261017.json) and
[variant archive](evidence/fargan-scheduler-variants-20261017-raw.tar.gz).
No hardware/compiler-layout mechanism was established. The selected design
keeps original field types and reservations; that storage fact is independent
of the measured timing result.

Ready publication uses bounded indexed eligibility checks. Packed selection
skips empty groups through its summary; it remains bounded by admitted
capacity, not asymptotically constant for every graph size. After all nodes
complete, Cord emptiness and abnormal-state inspection still have finite
active-prefix work; cancellation releases admitted ownership. These are named
remaining lifecycle costs, not a universal scan-free Step claim. No Play
implementation lookup, allocator, unbounded queue or hidden retry is added.

## Validation and retained evidence

On the after source: 246 kernel tests pass for each production/reference
selector, 338 AI tests and 12 ConduitOS timing/feedback tests pass, as do strict
kernel Clippy and no-default checking. The supported
`cargo xtask make conduitos timing-profile` proof retains 730 µs declared worst
cost under 1,000 µs, 480 µs met elapsed, and distinct met/miss/timer-loss/cancel/
stale outcomes. It is a deterministic-emulator proof with `physical_claim=false`.

The [current receipt](evidence/scheduler-fargan-completion-20261017.json) and
[raw archive](evidence/scheduler-fargan-completion-20261017-raw.tar.gz) reverify
after extraction: 288 FARGAN frames/76 hashes, 1,152 selector samples/67 hashes,
and 96 terminal pairs/24 hashes. All 24 corruption cases are refused, including
re-signed false work, output, Sign, allocation, storage and native-word claims.
The archive includes both internal wrappers, verifier snapshots, source-bound
logs and supported timing receipt. It contains no private model/oracle files.

These local records do not replace required Candidate and combined-development
proof. The PR and #5231 acceptance record retain those exact GitHub checks;
no stable-release or downstream speech-product acceptance is implied.

The first scheduler Candidate exposed the retained documentation corpus crossing its old 256-file evidence-validation cap (257 files). The [bounded capacity repair](evidence/scheduler-doc-capacity-20261017.json) raises that finite cap to 512, preserves the 4 MiB per-file/symlink/path checks, and tests acceptance at 512 and refusal at 513. Both original gallery/reference failures and all 56 evidence tests pass locally, as does strict xtask Clippy. This collector-only repair does not change the measured runtime.
