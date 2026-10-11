# Runtime performance reference worker

This is the first hosted measurement slice of #5395. It measures the existing
prepared expression evaluator and the production shared-pool selection function.
It does not yet answer the whole Plan/Play performance question.

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
and `--samples` (1,000–100,000, default 2,000). Repeat the command with a new
directory on an idle host before treating differences as repeatable.

## Timed boundaries

The frozen expressions perform 1/3/8/32 checked additions, a conditional, or a
comparison over U64 and the existing signed, six-decimal-place Scalar. The native
loop uses the same checked arithmetic and branches. Semantic tests include
branch boundaries, overflow refusals, the signed minimum, and malformed frames.
Both modes retain and compare the sum of all outputs for every repetition.
The native implementation uses registers; the prepared evaluator consumes
canonical eight-byte frames. That framing difference is disclosed, not removed.

Cold expression timings separate parsing, expression checking, portable-program
construction, evaluator preparation, validated first output, and teardown.
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

The regression policy remains report-only. Whole checked Plot/Plan/std Back/
kernel execution, 1/8/32 actual Gear pipelines, streaming/pressure/fairness,
payload/fan-out/batch scaling, representative DSP, and reviewed repeatability
remain #5395 acceptance work. Browser/WASM follows hosted repeatability.
Scheduling need not wait for a symbolic mathematics engine: existing fixed-width
paths can be measured now; reliable bounded arithmetic and type-safe projections
are the numeric integration gate.
