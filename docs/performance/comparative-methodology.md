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
repeat in `raw.ndjson`, derived `summary.json`, and preparation/steady-state
tables in `report.md`. `commands.txt` records every resolved runner invocation,
and the exact manifest and raw schema are copied beside the results. Metadata binds the commit,
fixture digest, machine, CPU, kernel, Rust, Node, Java, and pinned comparison
dependencies. Warm-up repeats are retained in the raw artifact; they are never
silently discarded.

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

The overload slice does not claim #245's persistent-session matrix.
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

## Regions and metrics

Assembly, exact-plan sealing where applicable, Start/subscription where it can
be separated, and steady execution are distinct fields. A synchronous RxJS or
Reactor subscription executes the graph inside the subscription call, so its
Start time is `null`; fabricating a separate value would change the graph.
Steady time, process CPU, resident memory, outcome accounting,
queue/value/evidence high water, post-Start allocations, and sampled end-to-end
latency accompany every repeat. Outcomes always separate offered, admitted,
completed-useful, rejected, sampled, coalesced, dropped, retried, and terminal
values. Overload rows additionally split the slow pressure region from the
recovery-to-terminal region. Unsupported measurements remain `null` with a
reason instead of being reported as zero.

The summary reports p50, p95, p99, p99.9, and maximum sampled latency. Useful
outputs per second is summarized across at least nine measured repeats with a
deterministic 10,000-resample percentile-bootstrap 95% interval. These noisy
wall-clock values are report-only until a reviewed machine-class baseline and
broad alarm are committed. Exact output and loss conservation, schema identity,
bounded high-water facts, presence of both overload regions where the policy
does not terminate, and Conduit's zero-allocation-after-Start result are strict
checks.

## Claim boundary

“Faster” means only a statistically summarized result for the exact recorded
machine, toolchains, fixture, and observation mode. “Better bounded” refers to
an explicit semantic capacity or a measured high water, not elapsed time.
RxJS asynchronous pressure and unavailable measurements are not comparable.
No percentile, observed maximum, successful run, or chart is a deadline,
admission, safety, portability, constrained-target, or real-time proof. The
contract boundary in #136 remains authoritative.
