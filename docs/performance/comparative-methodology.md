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

Every case starts with the same integer domain, applies the same `+2` maps,
uses parity-preserving filters, retains source ordering except for each
runtime's reported merge policy, loses no accepted values, and observes the
same terminal path. Depth means logical map/filter/merge operators; latency
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

## Regions and metrics

Assembly, exact-plan sealing where applicable, Start/subscription where it can
be separated, and steady execution are distinct fields. A synchronous RxJS or
Reactor subscription executes the graph inside the subscription call, so its
Start time is `null`; fabricating a separate value would change the graph.
Steady time, process CPU, resident memory, accepted inputs, useful outputs,
queue/value/evidence high water, post-Start allocations, and sampled
end-to-end latency accompany every repeat. Unsupported measurements remain
`null` with a reason instead of being reported as zero.

The summary reports p50, p95, p99, p99.9, and maximum sampled latency. Useful
outputs per second is summarized across at least nine measured repeats with a
deterministic 10,000-resample percentile-bootstrap 95% interval. These noisy
wall-clock values are report-only until a reviewed machine-class baseline and
broad alarm are committed. Exact output counts, schema identity, bounded
high-water facts, and Conduit's zero-allocation-after-Start result are strict
checks.

## Claim boundary

“Faster” means only a statistically summarized result for the exact recorded
machine, toolchains, fixture, and observation mode. “Better bounded” refers to
an explicit semantic capacity or a measured high water, not elapsed time.
RxJS asynchronous pressure and unavailable measurements are not comparable.
No percentile, observed maximum, successful run, or chart is a deadline,
admission, safety, portability, constrained-target, or real-time proof. The
contract boundary in #136 remains authoritative.
