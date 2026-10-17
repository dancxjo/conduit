# Production std streaming measurement for #5395

The previous finite sequence runner drained kernel work between each input.
The new explicit stream entrance offers a bounded batch while prior values are
still executing, using the same std Back factories, sealed Fore routes, atomic
remote fan-out, kernel queues, fuel, cancellation and Sign storage. Causal
sequence behavior remains unchanged. Its finite profile admits 1–4,096 inputs
and 1–64 admission attempts per kernel turn; queue and byte capacity remain
Plan-owned. Preparation validates every input before Play. Full retries keep
the exact payload and per-Fore sequence. Closure waits for drained work.

Fourteen supported hosted cases measure 1,024 useful input values: matched
1/8/32-Gear 32-addition chains, one/four fan-out branches, queue capacity one/four,
batch one/eight, fast/synchronous-slow acknowledgement and 32/256-byte Text
identity payloads. The existing 256-byte checked Text envelope is preserved;
512 bytes refuses before execution. The slow sink is a real synchronous Host
acknowledgement sleep, not a background scheduler. OS oversleep is included.

The fixed-capacity collector timestamps every acknowledgement, preserves exact
per-branch order and outputs, reports observed gaps between each branch's
completed values, and checks every native control. The fused native and
iterator-library controls preserve canonical data and the same sink dwell.
Their omission of typed Plan/Sign transactions is explicit. Compilation and
conformance are outside measurements; mode order rotates by repetition.

Conformance covers actual ordinary streams, pressure, fan-out, storage stability,
mandatory output Signs, malformed frames, overflow, cancellation, unplanned
Fores and oversized Text. Ingress model tests also prove no admission-path
allocation under bounded pressure and preserve legacy causal sequencing.

Raw rows and the independent verifier retain/check source and exact Plan/Host/
Boot/Play custody, per-output hashes, terminal evidence, queue/batch counts,
latency offsets and percentiles, CPU/allocator/phase observations, native output
parity and canonical boundary byte counts. Internal-copy hardware counters are
unavailable, not zero. The performance gate stays report-only.

Measurements and immutable source-bound receipts follow below after the
supported clean-source capture. This slice does not close the parent issue.

## Source-bound local measurements

Clean measured Source: `162f2576bab33a2bfd599f3c1e63fcc4d4d65b50`. The
supported hosted command ran 21 release conformance tests, then retained 187
artifact hashes, 232 raw records and 221 independently checked repeated
distributions. All 42 stream rows pass exact output/identity/pressure/terminal
verification, alongside the original 27 pipeline rows. Preparation, Play and
post-Play partitions are verified for both families.

```sh
taskset -c 4 cargo +stable xtask prove runtime-performance --output <new-directory> --values 1000000 --repetitions 3 --decisions 1000 --samples 1000 --hosted-pipeline true
```

Every stream completes 1,024 ordered inputs and every declared branch, with zero
loss or duplication. Four-way fan-out has at most three other acknowledgements
between each branch's consecutive outputs in every observed trial. Actual Full
attempts are retained, not counted as dropped input. Input and output byte
counts describe canonical boundary traffic; internal copying remains separate.

| Workload (branches/chain/queue/batch/bytes/slow µs) | std full call mean, ms | std Play mean ± batch SD, ms | native fused control mean, ms | iterator control mean, ms | Full attempts per trial |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1/0/1/1/8/20 | 83.589 | 83.219 ± 0.381 | 73.160 | 73.125 | 1022 |
| 1/0/1/8/8/20 | 83.652 | 83.306 ± 0.227 | 73.137 | 73.108 | 2045 |
| 1/0/4/1/8/20 | 84.425 | 84.070 ± 0.502 | 74.498 | 73.580 | 1016 |
| 1/0/4/8/8/20 | 83.693 | 83.342 ± 0.018 | 73.182 | 73.316 | 2039 |
| 4/0/1/1/8/20 | 227.096 | 226.168 ± 1.051 | 73.331 | 73.705 | 7157 |
| 4/0/1/8/8/20 | 297.548 | 296.566 ± 66.171 | 74.013 | 74.548 | 8180 |
| 4/0/4/1/8/20 | 343.244 | 342.132 ± 5.685 | 75.622 | 73.747 | 7133 |
| 4/0/4/8/8/20 | 284.344 | 283.292 ± 46.388 | 73.169 | 73.155 | 8156 |
| 1/0/1/8/8/0 | 12.167 | 11.818 ± 0.183 | 0.068 | 0.056 | 2045 |
| 4/0/1/8/8/0 | 155.007 | 154.189 ± 2.316 | 0.146 | 0.167 | 8180 |
| 4/0/1/8/32/20 | 220.895 | 220.400 ± 0.496 | 73.148 | 73.114 | 8180 |
| 4/0/1/8/256/20 | 263.319 | 262.798 ± 71.021 | 73.783 | 73.125 | 8180 |
| 1/8/1/8/8/0 | 84.553 | 84.009 ± 1.164 | 0.072 | 0.054 | 16332 |
| 1/32/1/8/8/0 | 360.044 | 358.809 ± 1.352 | 0.072 | 0.060 | 64922 |

These are three batches in one worker process, pinned to CPU 4. A background
build in another checkout was observed, and frequency, thermal state and other
load were uncontrolled. All samples, including unfavorable ones, remain. Batch
SD is descriptive, not an independent-process confidence interval or a
calibrated regression gate. A larger ingress batch does not consistently lower
wall time in this capture.

Without sink sleep, the 1/8/32-Gear arithmetic streams take about
11.8/84.0/358.8 ms of Play per 1,024 useful outputs. Direct native controls are
much cheaper. These workloads expose substantial coordination, framing and
mandatory evidence costs, rather than establishing native-speed execution.
The 20 µs requested sink dwell is included in all slow native/std controls;
actual OS oversleep can dominate their wall time.

The fused native source executes 32 checked additions per input and shares the
result among equal branches; the iterator source executes 32 per branch. Both
produce the same ordered canonical outputs. Optimized compiled instructions
may fold these operations; no hardware instruction count is claimed. The std
source retains 32 logical additions per useful result. Text cases are identity
transport, not another arithmetic-throughput claim.

Eight rehashed mutations of copied captures are refused: payload, sequence,
pressure accounting, missing pressure witness, Boot identity, terminal state,
latency percentile and removal of the entire declared stream matrix. The
original capture remains untouched. Legacy capture verification still passes.
Raw archive, positive/negative receipts, process-activity disclosure, debug
conformance and strict Clippy logs, and derived samples share the prefix
`evidence/runtime-stream-20261017-`.

A subsequent test-only change strengthens empty-output refusals and adds
cancellation after acknowledged output under actual fan-out pressure. The
measured production implementation is unchanged. Expanded independent-process
repeatability, DSP/control, numerical projections and the eligible
specialization/fusion matrix remain #5395 work. No PR or CI requested.
