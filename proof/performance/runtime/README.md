# Runtime performance reference worker

This hosted measurement slice of #5395 measures the existing prepared evaluator
and production shared-pool selection function. An optional profile adds ordinary
checked Plot/Plan/std Back/kernel execution through acknowledged typed Fores.

From a clean, committed checkout:

```sh
cargo xtask prove runtime-performance --output /tmp/conduit-runtime-reference-01 --repetitions 3
```

The directory must be new. The default is `target/runtime-performance`. Reusing
a directory refuses rather than overwriting evidence. Compilation happens before
worker measurements. The command records source custody before and after the
worker, retains stdout/stderr even on refusal, and hashes the retained artifacts
in `status.json`. A failed worker or changed source refuses the result. Keep all
files together; raw samples are in `raw.jsonl`, summaries in `report.json`, and
the exact authored expressions in `sources/`.

Finite options are `--values` (1–10 million, default 1 million),
`--repetitions` (3–15, default 5), `--decisions` (1,000–1 million, default 10,000),
and `--samples` (1,000–100,000, default 2,000). Add `--hosted-pipeline true` to
include the finite ordinary hosted trials. The driver selects the corresponding
worker feature and retains the exact build command. Repeat with a new directory
on an idle host before treating differences as repeatable.

## Timed boundaries

The frozen expressions perform 1/3/8/32 checked additions, a conditional, or a
comparison over U64 and the existing signed, six-decimal-place Scalar. Addition
trees balance the constant terms to fit the ordinary portable depth profile;
the checked program retains the declared number of additions. The native
loop uses the same checked arithmetic and branches. Semantic tests include
branch boundaries, overflow refusals, the signed minimum, and malformed frames.
Both modes retain and compare the sum of all outputs for every repetition.
A separately measured validation pass checks every output against the native
law and retains an order-sensitive SHA-256 of normalized eight-byte coordinates.
It is outside the bulk and event latency measurements. Primitive-operation
counts include comparisons; the conditional executes one comparison and one addition.
The native implementation uses registers; the prepared evaluator consumes
canonical eight-byte frames. That framing difference is disclosed, not removed.

Cold expression timings separate parsing, expression checking, portable-program
construction, canonical codec roundtrip, evaluator preparation, validated first
output, and teardown.
They exclude canonical Plot expansion and Plan construction. Warm bulk timings
exclude preparation and 10,000 warm-up values, and contain no per-value clock
calls. A separate uniform-stride pass retains event latency samples. Native and
prepared order alternate between repetitions. The timer-control distribution
is recorded without subtracting it from observations.

Selection trials call `admit_selected_pool_member` with 1/8/32 realizations and
1/4 resource requirements. Every trial uses fresh finite pool storage and the
current production capacity/utilization/Plan-order policy. The per-decision
clock includes validation, exact observation selection, selected Sign capture,
and atomic occupation. It excludes fresh-pool setup and subsequent activation
and release. A separate whole-loop wall/CPU/allocation measurement includes
these operations and output validation. All four lifecycle Signs and selected
observation Sign identities are checked. Missing or stale evidence must refuse
without occupation. The exact lowered fixture fields are retained.

The hosted profile holds useful work constant at 32 checked additions per U64
value, spread across 1, 8, or 32 actual Gears. It runs the normal production
kernel and standard expression Backs from a sealed Plan, with 64 finite ordered
input values and acknowledged outputs. Each repetition retains its source,
complete Host advertisement, Plan, active Play identity and kernel Signs.
Every output must match the native law, normalized ordered digest, exact Type,
port, track and sequence. Mandatory output Signs and fixed value-storage
capacity are checked. The equivalent native loop consumes and emits canonical
eight-byte frames; legal native compiler fusion is allowed and disclosed.

Cold hosted phases separate parsing, checking, expansion, Host-offer preparation,
realization selection, and Plan realization validation/sealing. The measured
Host call includes reservation, Back preparation, Play, Sign/report translation
and teardown. Its first-to-last output interval spans 63 additional completed
values; two wall samples in the Fore callback witness that interval. It is a
finite completion window, and the whole call is not labeled steady Play.
Host/harness teardown is measured separately. Allocations by the ordinary
Host path are reported, while output validation and artifact serialization stay
outside execution timings. These trials do not claim a million-item continuous
stream or independent cold Back-preparation timing.

## Interpretation and remaining acceptance

The report records actual wall time, Linux process CPU time, System allocator
calls, heap high-water, resident high-water, binary size, compiler, source,
hardware, topology, frequency observations, affinity, raw samples, and p50/95/99.
Process CPU time is null on unsupported systems. Per-selection CPU isolation,
hardware counters, internal-copy accounting, and independent-run confidence
intervals are not claimed. Heap peaks include retained harness metadata.
Background load and frequency changes remain uncontrolled. Allocator counters
run only on allocation/deallocation; allocation-free operations do not acquire
an instrumentation counter on each arithmetic operation.

The regression policy remains report-only. Fully separated cold Back/Play phases,
continuous streaming/pressure/fairness, payload/fan-out/batch scaling,
representative DSP, and reviewed repeatability
remain #5395 acceptance work. Browser/WASM follows hosted repeatability.
Scheduling need not wait for a symbolic mathematics engine: existing fixed-width
paths can be measured now; reliable bounded arithmetic and type-safe projections
are the numeric integration gate.

The same command also times the production Planner's typed fulfillment-cost
selector for 1/8/32 Hosts. The fixture checks the std pulse Kind through its
ordinary semantic catalogue and reuses the product std pulse Offer. Setup,
queue, service and transfer are exact Time coordinates supplied by the fixture;
they are not measured pulse performance. Hard resource observations, forecast
sources, an expired historical execution assumption and a separate fresh
current observation retain distinct producing Signs, Host/Boot/generation,
profiles and same-Host clock windows. All candidates must have known cost;
the deterministic winner follows the smallest exact fulfillment interval.

Each repetition performs 1,000 untimed, validated selection warm-ups. The
per-decision clock wraps `select_realization_with_costs`, including ordinary
hard admission, evidence/clock/condition validation, bounded exact comparison,
and construction of the returned candidate account. Receipt checks and account
destruction follow the clock. Whole-loop wall/CPU/allocation counts include
those operations. Once-per-fixture parse/forecast preparation and per-repetition
evidence-binding wall time are separate. Arithmetic work is reported as admitted
limb work alongside measured nanoseconds; the two are not interchangeable.
The exact fixture and every latency sample are retained. No live provider
residency or service measurement, dispatcher cost policy, global placement,
per-decision CPU counters, or zero-allocation planning claim follows from this
isolation baseline.

The hosted profile also measures typed cost decisions over 1/8/32 complete
production-owner reference std inventories, alongside the single-pulse-Offer
cases. Retain every published Offer and the original profile/resources; record
Offer counts separately from compatible candidate counts. Current timer-capacity,
clock, execution-condition and forecast facts remain controlled fixture evidence.
These timings measure the production Planner, including full inventory scans;
they do not measure physical model service, actual machine load, or distributed
execution. The same `--hosted-pipeline true` entrance enables these cases.

The supported command runs the release worker's conformance tests before any
measurement worker starts, retaining conformance command, exact source head,
exit status and stdout/stderr. With hosted pipelines enabled, this includes all
1/8/32-Gear pipelines, exact output/terminal custody and malformed/overflow input
refusal. Expression tests cover both U64 and Scalar 1/3/8/32-operator shapes,
branches, boundary inputs and malformed frames. Conformance timings are discarded;
a failed preflight retains its evidence and starts no measurements.

## Repeated batches and report-only CI

`report.json` includes `repeated_batches`, grouped by workload, execution mode
and timed metric. Each group retains every batch sample, mean, nearest-rank
percentiles and Bessel-corrected sample standard deviation. A single observation
has no sample standard deviation; unavailable CPU observations remain absent.
Event-latency distributions describe within-batch observations separately.
These are descriptive statistics from one worker process, not an independent
process confidence interval. Timer overhead is retained rather than subtracted.

Candidate CI runs the same finite hosted command on its exact merge preview,
with three repetitions, one million expression values and 1,000 selection and
latency samples. The independent job is report-only and does not participate in
the required candidate gate. Its artifact retains raw samples, source/Plan/Play
receipts, metadata, checksums and refusal logs for fourteen days. Download it
before expiry. Failure or missing measurements cannot establish a performance
result; thresholds require later calibration on controlled machines.

Native/prepared expression ratios describe checked arithmetic versus canonical
frame interpretation. Hosted/native ratios additionally include coordination,
reservation, Back preparation and retained Sign/report work, as disclosed in
each raw row. Compare matched workloads and clock scopes; neither ratio is a
universal claim of native speed. Cold Plot/Plan phases and the hosted full-call
boundary remain separate from evaluator warm bulk measurements. Finer inside-
kernel cold/warm phase measurements remain the distinct #5536 proof surface.

## Allocation window after actual execution begins

Hosted raw rows also retain `post_first_output_allocations`: process allocator
counter differences between the first and last real Fore acknowledgements. This
is the same finite 63-additional-output window as the callback wall samples,
after Play has begun, separated from earlier Back/Plan preparation. It includes
Host and collection work, not exclusively kernel allocation. Missing snapshots
refuse the measurement rather than producing zero. These counters are not a
whole-Play peak or a count of bytes physically copied.

For independent-process attribution, repeat the public command with three new
output directories and the same committed clean source. Keep raw counter samples,
exact source/Plan/Play/Boot custody and uncontrolled-host variation. The separate
fixed-kernel scan/copy entrance is documented in
[`kernel-overhead-attribution.md`](../../../docs/audits/kernel-overhead-attribution.md).

## Physical pure-chain dispatch

The hosted profile also runs the unchanged 1/8/32-Gear authored U64 pipelines
with the installed local computational Back, both without fusion and with the
reviewed `std/pure-chain-dispatch@1` alternative. The original hosted Host Call
baseline remains in the same run. Local/fused trial order alternates, every
output and mandatory output Sign is checked, and each exact Host/Plan/Play is
retained. A singleton retains its ordinary realization.

Fusion changes ready selection, not expression evaluation: the existing prepared
evaluator, all logical Gear/Cord identities, per-Gear transactions, source spans,
canonical intermediate values and bounded queues remain. Raw rows separately
report physical scheduler decisions, fixed-fixture intermediate materializations
and bytes, value-storage capacity, cold fusion selection, full-call wall/CPU,
first output, completion-window throughput and allocator observations. The
materialization counts describe this exact fixture; an internal-copy hardware
counter is explicitly unavailable. No transport elimination or zero-copy gain
is claimed.

Planner alternatives retain explicit addition/dispatch work assumptions and
zero assumed transport savings. They are not nanoseconds or measured provider
costs. Expensive fusion and required observation boundaries keep the unfused
choice. Target-specific timing observations stay separate from these policy
assumptions. Browser/WASM refuses this physical profile; native kernel tests do
not establish browser execution parity.

Region binding requires the kernel's `alloc` preparation feature, matching the
reviewed installed std Back. No-alloc products refuse binding and retain one
logical Step per dispatch. This keeps their fixed private image bounds intact;
it does not introduce allocation during admitted Play.

## Bounded overlapping stream measurements

The hosted profile additionally measures 1,024 inputs through the ordinary
checked Plan, std expression Backs and production kernel. Fourteen cases cover
1/8/32-Gear arithmetic chains with 32 useful additions per result; one/four
fan-out branches; queue capacities one/four; ingress batch budgets one/eight;
32/256-byte Text identity payloads; and fast versus synchronous slow output
acknowledgement. The existing 64-item causal sequence trials remain unchanged.

The std Host's explicit finite stream entrance validates at most 4,096 inputs
before Play and performs at most 64 atomic ingress attempts per kernel turn.
It preserves the current typed Cord capacities and pressure/closure rules.
The stream's order within every Fore is exact; its overlapping input cadence is
distinct from the existing drain-between-inputs causal sequence. All value,
identity and Sign storage is admitted before Play. This does not claim the
whole std Host allocates nothing after Start.

A slow sink sleeps for 20 microseconds in acknowledgement of output0. Actual
sleep duration is OS-controlled and remains in the measurements; it blocks
this cooperative Host, rather than inventing an asynchronous scheduling model.
The native fused-loop and iterator-library controls retain the same canonical
payloads and sink dwell, but omit typed Plan/Sign transactions. Their trial
order rotates across repetitions. Per-output Instant observations are included
in Host timings. No timer overhead is subtracted.

Raw rows retain every output's branch/sequence/size/hash, observed event offsets
and inter-acknowledgement percentiles; per-branch completed-output gaps; actual
accepted/full ingress attempts; full-call and installed phase wall times,
process CPU and allocator counts; first-to-last output allocation snapshots;
canonical boundary byte counts; Source/Host/Plan/Boot/Play and terminal evidence.
A boundary byte count is not an internal-copy counter. Fairness describes the
observed finite cooperative run, not a preemption or priority guarantee.

The capture verifier additionally checks the complete fourteen-case matrix,
exact authored arithmetic/Text source, every ordered output digest, source/Plan/
Host/Play custody, retained mandatory output Signs, bounded storage, pressure
witnesses and timing statistics. Three independent expanded captures are still
needed for expanded-workload process variation; one capture does not establish
a calibrated performance gate. DSP/numerical-projection and four-mode coverage
remain parent #5395 acceptance work.
