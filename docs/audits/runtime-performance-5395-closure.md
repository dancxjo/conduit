# Runtime performance #5395 closure audit

The exact parent issue is the acceptance contract. This ledger distinguishes
current local evidence from missing work; children being closed do not prove
parent completion. Operator instruction: work directly on dev, no PRs and no
CI requests. The performance regression gate remains report-only.

| Required surface | Current evidence | Remaining proof |
| --- | --- | --- |
| 1. Cold parse/check/expansion/selection/sealing/Back/first output/teardown | Supported runtime command retains cold expression and Plot/Plan phases; installed phase observer partitions preparation, Play and report cleanup | Consolidate current phase data and first-output scope in final report |
| 2. Million-value exact U64/Scalar 1/3/8/32 adds, conditions/comparisons versus optimized native | Frozen authored expressions, checked operations, raw timings/latencies, every-output ordered hashes; scalar tape audited separately | Consolidate baseline and specialization boundaries without conflating frame/native work |
| 3. Production Plan/Play 1/8/32 Gears versus native and equivalent pipeline | Ordinary std Host Call, local unfused and fused paths, same 64 inputs, retained Source/Host/Plan/Play/Signs; independent process evidence | 1,024-item 1/8/32-Gear streams now measured; complete paired specialization/fusion reporting where eligible |
| 4. Streaming/pressure | Fourteen ordinary 1,024-input overlapping cases retain exact pressure/fan-out/queue/batch/payload/acknowledgement/fairness observations; 42 verified trials in one process | Repeat expanded workloads in independent processes and consolidate uncertainties and synchronous-sink scope |
| 5. Representative domain | Product Theremin and ordinary audio execution have conformance tests | Measure deterministic admitted DSP/control workload through normal Plan/Play, validate useful outputs |
| 6. Numerical meaning | Fixed-width exact computation and separate arithmetic refusal tests exist | Installed #5531 already executes rational and certified binary64 projections of the same 0.3 Source; add paired measurements with receipt/refusal validation |
| Metadata and counters | Source/config/toolchain/CPU/affinity/frequency/OS, raw wall/process CPU, allocations, memory, size, Signs and decisions retained | Account copy/instruction/branch counters where supported, distinguish unavailable from zero, disclose observation and framing |
| Repeatability and report | Three independent pinned worker processes with descriptive process-mean SD, report-only gate, retained failures | Repeat expanded workloads and publish the plain-language answer and uncertainty |
| Browser | Browser performance remains explicitly deferred in existing receipts | Document the scoped subsequent browser/WASM gate once expanded hosted repeatability is demonstrated, as parent acceptance permits |

Closure requires equivalent checked outputs, refusals, ordering, pressure and
mandatory evidence across configurations; no benchmark scheduler, silent check
disablement, native-speed claim, or synthetic-work-unit timing substitution.
Compilation stays outside timed intervals. Raw evidence and verifiers must be
inspectable and bound to the measured source; later dev commits do not change
that identity. Hardware/physical/stable publication remain separate proof
classes. This ledger itself is not completion evidence.

The ordinary finite input feeder currently admits at most 64 values and waits
for kernel ready work to drain before admitting another. Repeating this fixture
does not prove a saturated streaming workload. Preserve this causal sequence
behavior; use an explicit admitted streaming surface for overlapping ingress
and pressure measurements rather than silently changing existing sequences.
Hardware events were probed locally and refused under perf_event_paranoid=4;
retain that unavailable counter evidence with the final report.

The explicit bounded stream implementation and verified first capture are in
[runtime-streaming-performance.md](runtime-streaming-performance.md). The
legacy causal feeder retains its 64-item drain-between-inputs behavior; it no
longer defines the limit of the measured Host streaming surface.
