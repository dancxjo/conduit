# Comparative reactive-runtime methodology

The comparative suite measures Conduit, RxJS, and Reactor without turning a
measurement into a semantic guarantee. The exact workload manifest is
[`benchmarks/comparative/manifest.json`](../../benchmarks/comparative/manifest.json).
Run it from a clean checkout with:

```sh
bash benchmarks/comparative/run.sh target/comparative-benchmark
```

Compilation and dependency installation finish before any timed region. The
runner refuses to overwrite an earlier result. It emits `metadata.json`, every
repeat in `raw.ndjson`, derived `summary.json`, preparation/steady-state tables
in `report.md`, and a separate `regressions.json`/`regression-report.md`
evaluation. `commands.txt` records every resolved runner invocation, and the
exact manifest, raw schema, and regression policy are copied beside the results. Metadata binds the commit,
fixture digest, machine, CPU, kernel, Rust, Node, Java, and pinned comparison
dependencies. It also records an explicit execution-environment machine class;
local runs default to `local-unclassified` rather than borrowing the CI gate.
Warm-up repeats are retained in the raw artifact; they are never silently
discarded.

Pinned JavaScript packages are installed in a generated sibling under
`target/`, never beneath the source tree; repository-wide schema gates therefore
cannot accidentally scan third-party package fixtures.

Each raw row carries the shared logical-fixture identity. Conduit rows also
carry the exact plan identity, source semantic hash, and digest of the executed
benchmark binary. RxJS, Reactor, and language-loop rows leave Conduit-specific
identity fields explicitly `null` instead of fabricating plan identities.

## What is compared

The local-depth cases start with the same integer domain, apply the same `+2`
maps, use parity-preserving filters, retain source ordering except for each
runtime's reported merge policy, lose no admitted values, and observe the same
terminal path. Depth means logical map/filter/merge operators; latency
observation taps and sinks are reported separately and are not counted as
operators. Rust, JavaScript, and Java identity loops are emitted in separately
labelled lower-bound tables. They isolate language-loop cost and are never
reactive-runtime competitors.

The current Conduit sample exercises the public exact-plan validator and
`DeterministicExecutor`, including bounded FIFO cords, lifecycle transitions,
fixed scheduler evidence, and explicit evidence-log acknowledgement. Its
handle-backed `u64` driver isolates reference-scheduler cost. It is not an
optimized hosted-streaming result and the report never substitutes it for one.
That mode remains unavailable pending the persistent and portable execution
work tracked by #214 and #242. For the same reason, the bounded asynchronous
case currently contains only Reactor results; neither Conduit's single-lane
reference runner nor RxJS is relabelled as a demand-bounded asynchronous mode.

RxJS is the matched JavaScript push comparison. Its synchronous cases have no
Reactive Streams demand contract. The suite marks the bounded asynchronous
case unavailable instead of creating an uncontrolled `observeOn` queue or
inventing demand. Reactor is the demand-aware comparison; its `publishOn`
prefetch is pinned to the manifest queue capacity. Default fusion and batching
are reported rather than disabled.

The finite overload slice drives the Conduit reference scheduler through a
single exact cord at capacities 4, 64, and 1,024. A consumer yields three
bounded scheduler quanta per value until twice the cord capacity (or half of a
short source) has completed, then clears the delay so the artifact contains
separate pressure and recovery regions. It runs block, reject,
latest-wins coalesce, deterministic sample, type-declared disposable drop,
disconnect, and fail policies. The runner reports every policy outcome
separately. A coalesced admission replaces one queued value and does not count
as a useful completion; a sample-selected value that still cannot fit is a
drop, distinct from schedule exclusion. Disconnect and fail stop at the first
saturated offer and are terminal measurements, not throughput successes.

The bursty-consumer slice is separate from that one-way slow-to-recovered
shape. At capacity four, one selected slow consumer begins with eight explicit
cooperative pause yields, consumes a burst of eight values, and repeats that
bounded pattern. The five nonterminal pressure policies run against the same
pattern; terminal disconnect and fail remain in the sustained matrix because
they intentionally stop at first saturation and therefore cannot exhibit
repeated bursts. Coupled and isolated fan-out repeat the bursty pattern at
2, 8, and 32 branches with both one and all branches slow. Raw rows identify
the consumer pattern and burst size, while exact pressure/recovery cycle counts
describe the repeated regime. The report leaves the single contiguous
`pressure_ns`/`recovery_ns` pair null for bursty rows instead of pretending
several alternating regions are one phase.

RxJS overload remains unavailable because synchronous push has no matching
demand-bounded queue. Reactor overload also remains unavailable until its
demand, buffer, and loss mappings receive a semantic review; the existing
local-depth `publishOn` case is not substituted for either comparison.

The fan-out slice covers coupled and isolated publication to 2, 8, or 32 exact
branch cords at capacities 4, 64, and 1,024, with both one slow branch and all
branches slow. Coupled admission reserves every branch in one scheduler
transaction; a pressured attempt rolls back all earlier reservations. Isolated
publication uses an explicit ordinary duplicator node. It removes one input
from its own finite cord, retains at most that one value under a `Retained`
memory claim and execution-profile limit, and publishes it to each finite
branch cord in independent transactions. Its per-branch progress is a fixed
32-entry driver field, not a queue or an unbounded adapter.

Useful completion counts branch deliveries in both modes, so the strict
invariant is admitted inputs multiplied by branch count. Every cord's observed
item high water is checked against its declared capacity; aggregate high water
also includes the isolated duplicator's input cord. RxJS and Reactor fan-out
comparisons remain unavailable until their multicast, demand, buffering, and
coupling semantics have reviewed mappings.

The shared-payload slice is a separate #244 production-hosted boundary. It
authors one 1 KiB or 1 MiB public-text `std/literal` and 2, 8, or 32
`display/text` consumers. The hosted literal profile pins a hard 32-reservation
output bound and derives its aggregate output and shared-handle representation
bounds from the exact configured literal bytes. The display host-I/O profile
derives its per-value bound from the exact cord topology. Both reject values
above the reviewed 1 MiB ceiling. Because the current checked-in compile
document cannot yet represent `PlanFanOut`, the benchmark compiler seals the
one-branch provider/profile skeleton and the harness assembles the exact
current core fan-out fact for the full source-derived topology. The production
registry still resolves every authored node, production drivers execute every
step, and the raw row binds the full source semantic hash, recomputed plan
identity, implementation artifact, and exact scheduler evidence. This is
runtime evidence, not a claim that the general compile-document producer now
supports fan-out.

The benchmark caller supplies a host snapshot whose available-memory fact and
plan budget cover the selected payload/branch matrix before sealing. Panel
source cannot manufacture that host fact. The raw sample separately reports
the executor's exact planned allocation and the process resident-memory
observations; neither is presented as physical admission evidence.

Every branch receives the same opaque generation-safe arena handle. The runner
requires one unique branch handle, one resident value slot and exactly 1,024 or
1,048,576 resident value bytes at high water, exact 2/8/32 branch-delivery
counts, zero value residency after terminal completion, and zero allocator
calls after Start.
Each display sink checks the actual payload bytes. Its preallocated host-output
storage therefore scales with branch count and payload size, and each
capacity-one cord separately charges the exact payload capacity even though
all cords name one arena value. Those verifier and queue charges are reported
beside arena residency; the suite does not call the end-to-end path zero-copy.

The same 2/8/32 matrix has a distinct Abort case. One bounded scheduler
decision performs the exact atomic coupled publication, leaving the same handle
on every capacity-one branch cord. The host then requests Abort before a sink
can consume. The runner requires the terminal class to be cancelled, zero
branch deliveries and verifier bytes, the full branch-count-times-payload
queue-payload high water, zero allocator calls after Start, and zero value
slots/bytes after terminal cleanup. Completion content verification and
cancellation reclamation are separate measurements rather than a cancellation
run pretending it consumed payload content.

Each completion and Abort row also runs with no Watch, one watched branch, and
every branch watched. Watched rows admit exact cord subjects into the plan and
attach them before the timed region. Each admission has one fixed `Latest`
record and a 64-byte public preview buffer, both included in planned/runtime
memory accounting. A Watch observes only a committed production cord
publication; it does not add demand, delay a cord, or change the graph.

After terminal state, the runner first requires zero resident value slots and
bytes, then reads each Watch outside the timed allocation scope. The read must
return one record carrying the same generation-safe handle, the verified
64-byte payload prefix, the full content hash, and a truncation marker.
Abort therefore retains the separately copied preview even though the original
arena value and every queued reference have been reclaimed. The report exposes
admitted and attached slots, retained records/bytes, drops, and maximum fixed
Watch storage so copied observability cost cannot be mistaken for executor
value residency or zero-copy delivery.

The labeled copy-required comparison currently covers 1 KiB public text with
2, 8, and 32 branches. It keeps the same exact coupled shared-handle
publication at the source boundary, then places one production
`text/uppercase` node and one `display/text` sink on every branch. Each
uppercase driver reads the source handle, allocates its accounted 1 KiB
transformation buffer after Start, stores the transformed bytes in the fixed
generation-safe arena, and publishes a distinct output handle. Full uppercase
content verification happens outside the timed region. Raw rows require
exactly the branch count in shared source-handle publications, branch copy
operations, after-Start allocation calls, and distinct branch output handles;
copied and allocated bytes both equal branch count times 1,024. Together with
the source, unique handles equal branches plus one. Scheduling reclaims each
branch copy promptly, so value-store high water remains exactly two slots and
2,048 bytes while terminal residency returns to zero. This is a plan-visible
production branch transformation, not a claim that the runtime executes
`DuplicationRule::Copy`.

This slice does not substitute payloads above 1 MiB, PCM, images, encoded
frames, fragments, copy-required payloads above 1 KiB, isolated subscribers,
ring/sample Watch retention,
mid-run attach/detach/reconnect, coalescing, slot reuse, or browser execution.
The current hosted literal value binding is bounded to 1 MiB, and RxJS/Reactor
object references have no reviewed mapping to Conduit's generation-safe handle,
Watch, and arena-residency evidence. Those cases remain explicitly unavailable
and #244/#248 remain open.

## Regions and metrics

Assembly, exact-plan sealing where applicable, Start/subscription where it can
be separated, and steady execution are distinct fields. A synchronous RxJS or
Reactor subscription executes the graph inside the subscription call, so its
Start time is `null`; fabricating a separate value would change the graph.
Steady time, process CPU, resident memory, outcome accounting,
queue/value/evidence high water, post-Start allocations, and sampled end-to-end
latency accompany every repeat. Outcomes always separate offered, admitted,
completed-useful, rejected, sampled, coalesced, dropped, cancelled, retried, and terminal
values. The Conduit runner also reports exact scheduler decision count and wall
time accumulated only while a source is blocked waiting for bounded output
capacity. Overload rows additionally split the slow pressure region from the
recovery-to-terminal region. Bursty rows instead carry exact repeated
`pressure_cycles` and `recovery_cycles`; terminal scheduling can leave one
more entered recovery cycle than completed pressure cycle. `drain_ns` and
`abort_ns` remain `null` until a
fixture explicitly requests the corresponding cancellation transition;
ordinary successful completion is not renamed. Other unsupported measurements
remain `null` with a reason instead of being reported as zero.

The cancellation slice requests `Drain` and `Abort` only after a capacity-four
FIFO cord has entered block pressure. Drain keeps consuming every value already
admitted before the request; Abort may discard admitted queue contents, which
remain excluded from completed-useful. The request-to-terminal duration is
reported only in the matching `drain_ns` or `abort_ns` field. These are finite
exact-run cancellation measurements, not persistent-session results.

The persistent overload slice uses the production `ExactRunSessionRegistry`
and `ExactRunSession`, not a benchmark-owned loop renamed as a session. One
admission reserves the finite runtime-memory budget before Start and remains
owned across repeated host pumps of at most eight scheduler decisions. The
source has a standing wait at the configured observation-offer boundary rather
than natural completion. FIFO Block requests termination when the final offer
is pending against a full cord; the non-blocking policies resolve every offer,
enter the standing wait, and then request termination. In both cases a positive
bounded number of admitted values remain pressured. The exact cancellation wake
lets a waiting source observe the request and complete. The raw row reports
pump count, retained reservation bytes, and pressured items at the request.
Drain preserves retained admitted work;
coalesced replacements remain separately excluded. Abort leaves
admitted-but-aborted work outside useful completion. Terminal finalization
returns the scheduler and releases the session admission; the runner checks
both active-session and reserved-byte counts return to zero. The matrix repeats
this ownership boundary for FIFO
block, reject, latest-wins coalesce, deterministic sample, and type-proven
disposable drop. Disconnect and Fail are excluded because they terminate on
the first saturated offer instead of reaching a standing observation boundary.
For these rows, `input_values` names the bounded observation offer window, not
the standing source's lifetime cardinality; `session_mode` prevents the two
meanings from being conflated.

The persistent host-wake residency slice is a distinct #249 fixture rather
than another pressure-policy row. One release-profile production
`ExactRunSession` first reaches an exact named host-operation wait, then
receives 10,000 `benchmark/persistent-wake` notifications without a benchmark
reset. Each notification admits one handle-backed value to one finite FIFO
cord; bounded host pumps continue until the sink has consumed that value and
the same source is standing again. At wake 1,000 the runner records the
scheduler high water. Queue items, queue payload bytes, ready slots, and
mandatory scheduler-evidence slots must have exactly the same high-water values
after wake 10,000. A mismatch fails the run, so final-only reconciliation
cannot hide monotonic growth. The raw row binds the checkpoint, reports the
exact host-wake and pump counts, and marks the plateau proof explicitly.

The persistent timer residency row repeats the same 10,000-wake and
wake-1,000 checkpoint contract through the production timer boundary. The
source retains one exact `benchmark/persistent-timer` deadline at
`io.tick() + 64`, which stays ahead of the eight-decision pump quantum; the
host reads that retained deadline from the session and
calls `ExactRunSession::advance_to` exactly once per value. No wall clock,
sleep, interval adapter, callback queue, or session reset is involved. Its raw
row reports timer wakes separately from host-operation wakes and applies the
same exact checkpoint/final high-water equality and final Drain reconciliation
gate.

For both residency rows, the global allocator counter covers the complete
region after `DeterministicExecutor::start`: initial wait, every host
notification or exact timer advance and bounded pump, evidence acknowledgement,
requested Drain, terminal pump, and session finalization. Its reviewed target
for this exact reference-scheduler,
handle-backed driver, release-profile path is zero calls and zero bytes. The
fixtures also require every offered value to be admitted and consumed before
the next wake, then checks the session registry returns both active sessions
and reserved bytes to zero exactly once. It does not claim a hosted value arena,
provider buffer, worker pool, hosted timer provider, interrupt path, or all-Conduit
zero-allocation result; those #249 workloads remain open. Process RSS is
reported only as supplementary process telemetry and is never used to prove
the internal plateau.

The summary reports p50, p95, p99, p99.9, and maximum sampled latency. Useful
outputs per second is summarized across at least nine measured repeats with a
deterministic 10,000-resample percentile-bootstrap 95% interval. The separate
`regression-policy.json` carries its source workflow, artifact, merge, head,
observed CPU/kernel, exact applicability scope, baseline values, and broad
threshold rationale. The source artifact predates the explicit machine-class
field; the policy records that its class was derived from the cited workflow's
`ubuntu-latest` declaration and the artifact's `x86_64` observation rather than
from a developer-local assumption. Its current smoke policy applies only when
the recorded machine class is `github-hosted-ubuntu-x86_64`, the architecture
is `x86_64`, the input cardinality is 10,000, and warm-up/measured trial counts
are exactly 2/9. A local, full-cardinality, differently classified, or otherwise
mismatched run produces a `not-applicable` evaluation and remains report-only.

On a matching run, useful-throughput collapse alarms only when the current
bootstrap 95% median-confidence upper bound falls below half the reviewed
baseline. Conduit p99 alarms permit threefold growth. RxJS/Reactor p99 and all
p99.9 ratios remain visible but report-only: the policy's cited calibration
artifacts span AMD EPYC 7763/9V74 and Intel Xeon 8573C runners and show
outlier-heavy comparison-runtime tails that do not track useful-throughput
collapse. The selected groups cover Conduit/RxJS/Reactor map depth, Conduit
bounded overload, 32-branch coupled fan-out, and persistent host/timer wake
paths. The evaluator
fails the matching CI job if a selected group disappears, has fewer than nine
measured trials, or crosses a threshold; its artifact retains every ratio and
alarm for review. CPU and kernel observations are provenance rather than hidden
selectors, so runner-pool drift is visible while the broad thresholds absorb
ordinary variation. Passing or alarming never establishes a performance,
deadline, admission, safety, or portability guarantee.

Exact output and loss conservation, schema identity, bounded high-water facts,
presence of both overload regions where the policy does not terminate, and
Conduit's zero-allocation-after-Start result remain strict deterministic checks
on every machine.

## Claim boundary

“Faster” means only a statistically summarized result for the exact recorded
machine, toolchains, fixture, and observation mode. “Better bounded” refers to
an explicit semantic capacity or a measured high water, not elapsed time.
RxJS asynchronous pressure and unavailable measurements are not comparable.
No percentile, observed maximum, successful run, or chart is a deadline,
admission, safety, portability, constrained-target, or real-time proof. The
contract boundary in #136 remains authoritative.
