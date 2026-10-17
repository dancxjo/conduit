# Real Plan/Play arithmetic and coordination tax: #5395

On this host, tiny arithmetic through the prepared tree and real Plan/Play is
substantially slower than optimized native computation. Increasing arithmetic
inside one expression exposes recursive checked-operation/frame overhead;
spreading the same arithmetic across Gears adds dispatch, typed Cord and Host
coordination cost. These samples establish no arithmetic size at which the
reference becomes comparable to optimized native throughput. Slow sink dwell
can make wall times look closer while substantial CPU overhead remains.
This is an actual measured limitation, not a native-speed or BASIC comparison.

Expression specialization has a narrower useful execution profile, but its
ordinary Value Plays are slower end to end here. Reviewed Flow fusion reduces
kernel work without a demonstrated repeatable full-call wall gain. Preserve
both findings rather than choosing favorable timers or discarding regressions.

## Subject, custody and uncertainty

Measured clean Source: `a52ddeff5fc3e047bdf311591a1c7ef8233d69ee`.
Native Linux `forebrain`, i9-12900H, 14 cores/20 logical CPUs, CPU 4 affinity;
Linux 7.0.0-38-generic, rustc/cargo 1.98.1, LLVM 22.1.8, release profile,
debug assertions off, no RUSTFLAGS or release overrides. The unstripped hosted
measurement executable is 22,435,872 bytes. Exact build commands, compiler,
frequency snapshots/topology, OS, config, resource ceilings, sampling policy,
Source/Host/Plan/Boot/Play and implementation identities are in every archive.
The driver does not change affinity; `taskset` pins the wrapper and children.
Frequency, thermal state, interrupts and background development load remain
uncontrolled. Frequency snapshots are not locked-frequency evidence.

Captures 02/03/04 each contain three batches in a separately launched worker:
PID/start ticks 153622/45164745, 154511/45175243, 155372/45185853, observed live
through `/proc`. All had affinity 4 and executable SHA-256
`91feb9f4fed3b2f0c7ced76bc6ea28d9b28a161afc5909390c1d76e4829b9adb`.
Independent verification checked 726 artifact hashes, 768 raw rows and 741
within-process batch distributions (247 metric groups). Expanded verification
checks 3,681 distinct actual Plays and 93 matching executed workload/batch rows
per process, including exact Source and output parity. Diagnostic capture 01
also passes, but lacks a live worker observation: it is retained and excluded
from this cohort for custody, irrespective of its timing.

Tables show **mean ± sample SD of the three process means**, in milliseconds
unless stated. These are descriptive variation estimates, not inferential
confidence intervals or a calibrated gate. All batches, event samples and
outliers remain available. Displayed 0.000 SD means rounding below 0.0005 ms,
not zero variation. Each warm expression has one million values and
1,000 separately sampled events. Timer controls are retained without
subtraction. No numeric confidence level or causal historical speedup is
asserted on this shared host. The regression policy remains report-only.

## Warm expression: framing and checked dispatch

Both native and prepared perform the same checked U64/signed six-decimal Scalar
law. Every output is checked in a separately timed validation pass, with an
ordered normalized digest and sum. Warm bulk excludes preparation and the
explicit 10,000-value warm-up; it has no per-item clock. Alternating mode order
and a separate uniform-stride event-latency pass are retained. Source-level
primitive-operation counts include the comparison; a conditional executes a
comparison and one addition. Native register input and prepared canonical
8-byte frame input differ. The compiler legally folds native arithmetic;
these are source arithmetic counts, not retired machine instructions.

| Million-value workload | Native optimized | Prepared reference |
| --- | ---: | ---: |
| scalar-add1 | 2.011 ± 0.006 | 64.049 ± 1.479 |
| scalar-add3 | 2.007 ± 0.001 | 135.950 ± 0.050 |
| scalar-add32 | 5.032 ± 0.007 | 1195.615 ± 8.080 |
| scalar-add8 | 2.407 ± 0.000 | 316.853 ± 1.138 |
| scalar-comparison | 1.810 ± 0.002 | 63.762 ± 0.821 |
| scalar-conditional | 1.805 ± 0.002 | 123.111 ± 0.974 |
| u64-add1 | 2.013 ± 0.006 | 74.509 ± 1.249 |
| u64-add3 | 2.006 ± 0.000 | 169.085 ± 0.151 |
| u64-add32 | 2.001 ± 0.010 | 1555.294 ± 10.771 |
| u64-add8 | 2.007 ± 0.002 | 403.712 ± 0.556 |
| u64-comparison | 1.807 ± 0.000 | 68.210 ± 1.145 |
| u64-conditional | 1.802 ± 0.008 | 138.096 ± 1.268 |

The allocation-free warm evaluator remains an interpreter; allocation freedom
is not expression compilation. Native p50/p95/p99 close to the timer floor
must not be turned into reliable per-instruction costs. The retained event
histograms and process CPU measurements complement, rather than replace, the
bulk throughput boundary.

## Production std Host scaling and cold phases

Every finite trial is a normal checked/sealed Plan through product std Backs,
production kernel, typed cords, reservations and required Signs. Sixty-four
canonical U64 inputs each require 32 checked additions, distributed over 1/8/32
Gears. Native consumes and emits the same canonical frames. The stream matrix
below also includes an explicitly equivalent native library pipeline.

| Gears | Full Host wall | Process CPU | Back preparation mean | Play mean | Decisions | Signs |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 0.913 ± 0.061 | 0.914 ± 0.061 | 0.414 | 0.384 | 130 | 322 |
| 8 | 2.003 ± 0.171 | 2.003 ± 0.171 | 0.520 | 1.335 | 1296 | 546 |
| 32 | 6.238 ± 0.050 | 6.238 ± 0.049 | 0.934 | 5.032 | 5184 | 1314 |

Full-call useful rates are 64 divided by the reported full-call seconds, not
Gear activations. Canonical native loop means are 1.366/1.548/1.737 microseconds
for 1/8/32 Gears' equivalent computation; these tiny batches are timer-sensitive.
The 63-output first-to-last completion window is separately retained and is
not labeled an indefinitely sustained service rate.

The following cold means for the **same ordinary one-Gear fixture** are separate
wall measurements. Back preparation and Play above are inside the full-call
boundary; never add them again to that boundary.

| Cold boundary | Mean ms |
| --- | ---: |
| parse | 0.051008 |
| check | 5.042966 |
| expand | 0.146314 |
| host-offer-preparation | 0.047464 |
| realization-selection | 0.009940 |
| physical-fusion-selection | 0.000040 |
| plan-realization-validation-and-sealing | 0.050249 |

First acknowledgement from call entry averages 0.493 ms; final Host/harness teardown averages 0.014175 ms. The sum of these accountable cold phases, full Host call and teardown is 6.276 ms. This sum is not a new uninterrupted stopwatch: fixture generation, input/oracle
preparation, validation, metadata serialization and harness bookkeeping lie
outside those named execution boundaries. Expression-only cold rows additionally
separate parse, check, portable-program construction, canonical codec roundtrip,
evaluator preparation, first output and destruction. Compilation and release
conformance occur before measurements. No unreported execution warm-up is
hidden in cold timings; retained catalogs/harness data mean these are fresh
object preparations, not cold OS-cache or fresh-process startup estimates.

Installed phase markers partition full-call wall into pre-preparation,
Back preparation, Play and post-Play/report cleanup. These have wall clocks;
process CPU is reported for enclosing bulk phases, not invented per marker.
`evaluate_node`, primitive decoding/validation and arithmetic are combined in
the isolated expression measurement. Kernel byte-sink and Sign probes below
isolate coordination/evidence separately; they cannot be subtracted to invent
exact causal percentages for the full Host.

## Stream pressure, payload and batching

Each run preloads 1,024 inputs into a finite explicit admitted overlapping stream
(maximum 4,096, batch maximum 64). Queue choices 1/4, batches 1/8, branches 1/4,
and 8-byte U64 or 32/256-byte text payloads retain every accepted value and
required Sign. One slow sink uses synchronous 20-microsecond sleep on output0;
actual sleep/OS delay exceeds its requested dwell. This is cooperative finite
pressure, not an asynchronous producer or a hardware network benchmark.
Native controls use the same useful outputs, frames and dwell; they omit typed
Plan/Sign transactions and may reorder globally across branches. Per-branch
order/bytes and no loss must match. Full means include setup inside the Host
call; cold Plot/Plan is separate.

| Branches / chain / queue / batch / bytes / dwell us | Host wall | Native fused wall | Native library wall | Decisions | Useful outputs |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1/0/1/1/8/20 | 109.406 ± 1.981 | 80.245 ± 0.341 | 83.238 ± 0.882 | 2049 | 1024 |
| 1/0/1/8/8/0 | 11.812 ± 0.021 | 0.073 ± 0.000 | 0.046 ± 0.000 | 2049 | 1024 |
| 1/0/1/8/8/20 | 110.902 ± 0.341 | 80.399 ± 0.218 | 83.706 ± 0.031 | 2049 | 1024 |
| 1/0/4/1/8/20 | 110.348 ± 0.738 | 79.816 ± 1.146 | 83.708 ± 0.069 | 2049 | 1024 |
| 1/0/4/8/8/20 | 110.354 ± 0.383 | 80.940 ± 1.048 | 83.752 ± 0.022 | 2049 | 1024 |
| 1/32/1/8/8/0 | 340.653 ± 0.251 | 0.081 ± 0.001 | 0.053 ± 0.002 | 65631 | 1024 |
| 1/8/1/8/8/0 | 79.681 ± 0.057 | 0.087 ± 0.002 | 0.052 ± 0.001 | 16407 | 1024 |
| 4/0/1/1/8/20 | 232.015 ± 0.472 | 78.541 ± 0.094 | 82.105 ± 0.006 | 8200 | 4096 |
| 4/0/1/8/256/20 | 222.120 ± 0.686 | 78.573 ± 0.112 | 81.001 ± 0.066 | 8200 | 4096 |
| 4/0/1/8/32/20 | 224.105 ± 0.319 | 78.156 ± 0.430 | 81.661 ± 0.075 | 8200 | 4096 |
| 4/0/1/8/8/0 | 145.978 ± 0.045 | 0.211 ± 0.006 | 0.166 ± 0.006 | 8200 | 4096 |
| 4/0/1/8/8/20 | 231.320 ± 0.083 | 78.592 ± 0.146 | 82.159 ± 0.034 | 8200 | 4096 |
| 4/0/4/1/8/20 | 231.416 ± 0.180 | 78.065 ± 1.032 | 82.086 ± 0.108 | 8200 | 4096 |
| 4/0/4/8/8/20 | 231.942 ± 0.690 | 78.443 ± 0.287 | 82.121 ± 0.050 | 8200 | 4096 |

Full attempts are explicit pressure refusals, not losses or admitted duplicates.
For example, the first 02 queue1/batch1/single-sink sample accepts 1,024 of 2,046
attempts with 1,022 Full responses; it delivers all 1,024 ordered outputs.
Its inter-acknowledgement p50/p95/p99 is 108.123/119.372/125.402 microseconds.
Every sample retains offsets, quantiles, per-branch completion counts and
maximum other-branch acknowledgements between consecutive outputs. These
witness finite fairness, not priority/preemption guarantees. Latency starts at
Host call entry or adjacent acknowledgements, not producer hardware ingress;
per-delivery clock observation and sink dwell are included. Larger payloads
change supported Type/computation and may reduce unrelated U64 work; no isolated
byte-size causal ratio is inferred. Invalid 512-byte profile and cancellation
under pressure have refusing/no-later-output conformance tests.

## Eligible optimization matrix

No benchmark widens an installed profile. Flow reference/fusion use identical
Flow Source, values, checked meaning and mandatory output evidence; Value
reference/tape use identical Value Source and protocol. The families cannot
form a four-way timing ratio. Scalar tape actually refuses Flow with
`scalar tape profile requires Value temporal contract`. Combined Value/fusion
actually refuses with `pure-chain requires the exact installed computational
Offer` for 8/32 Gears; a singleton has no fusion region. Captures fail if this
eligibility changes without expanding the matrix. Unsupported is not zero time.

| Gears | Flow local unfused | Flow fusion selected | Value reference | Value scalar tape |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.791 ± 0.046 | 0.857 ± 0.036 | 10.058 ± 0.091 | 12.839 ± 0.047 |
| 8 | 1.751 ± 0.055 | 1.767 ± 0.031 | 18.014 ± 0.071 | 21.180 ± 0.223 |
| 32 | 3.433 ± 0.057 | 3.394 ± 0.078 | 58.560 ± 1.028 | 63.757 ± 0.768 |

Flow is a single finite 64-input Play. Value is **64 separate ordinary Plays**,
reusing the sealed Plan/Host with distinct increasing Play identities. All
reservation, Back preparation, typed transactions, Signs and report work remain.
Its batch is not warm evaluator throughput. Flow fusion reduces physical
regions/decisions with exact logical/Sign observability retained, but these
wall observations do not show a repeatable full-call gain. Value tape's cold
preparation outweighs any narrow evaluator benefit here. The earlier warm tape
experiments in [scalar-tape-scratch-20261017.md](scalar-tape-scratch-20261017.md)
remain attributed to their earlier source and different boundary.

## Instrument control and certified numerical meaning

The control fixture preserves the canonical Pocket Theremin pitch law. The
original reusable function has Value Fores; a retained explicit ordinary Flow
expression substitutes the one distance parameter with `.` without changing
the law. Original instrument, exact derived checked Source and input frames are
retained. Inputs cycle -5 through 35cm, alternating cm/mm Quantity frames.
All 1,024 exact clamped Hz outputs match a native control with the same Quantity
decoding, unit conversion and canonical framing. Wrong dimension/malformed
frames refuse. This measures useful pitch-control updates, not audio rendering,
cadence, playback or physical/human enactment.

Control full wall: **79.172 ± 0.319 ms**, process CPU mean 78.855 ms; native
canonical control mean 17.176 ms. Back preparation/Play means are 2.696/76.379 ms;
2,049 decisions, 4,162 retained kernel Signs. Full-call useful rate is about
12,934 control updates/s for this finite corpus, not an audio-rate promise.

Installed numerical Backs execute one exact authored `0.3` computation under
exact and loose error requirements. Source remains 3/10 in both receipts;
exact rational target/error is 3/10 and zero. Binary64-nearest target is
5404319552844595/18014398509481984, error 1/90071992547409920, nearest-even,
inexact=true, tiny=false, one step. Zero-error binary64 planning and forged
artifacts refuse. The verifier decodes canonical structured Types and normalized
rationals, recomputes profiles and checks bits/error; it does not trust a
validation boolean. Full Host wall: **224.122 ± 0.684 ms**, CPU mean 224.112 ms.
Preparation mean 224.046 ms, Play 0.012917 ms: these product Backs prepare/freeze
receipts before Play. Tiny Play time is receipt delivery, not numerical
throughput. Type-catalog setup (~33.447 ms), source-specific certified-offer
preparation (~117.256 ms) and isolated actual Back preparation are separate
retained cold measurements, not silently excluded arithmetic.

Certificate accounting is distinct from clocks/allocators: rational prepared
limb work 0 / independent validation 8,704; binary64 prepared 1,446,201 /
validation 1,456,953, each admitted maximum 100,000,000. Retained payloads are
4,212/12,556 bytes. The isolated Back-preparation allocator records the real
symbolic/certificate work separately from normal Host/Plan overhead; no hot
bignum or symbolic arithmetic throughput claim follows.

## Allocation, copies, memory and Sign cost

All bulk phases retain Linux process CPU, successful System allocation and
reallocation counts, requested bytes, live/peak heap, resident high-water and binary size.
Process resident high-water is 369,064/369,232/369,224 kB across the cohort.
Heap peaks include retained harness metadata and are not exclusive product RSS.
Post-first-output windows begin at the first actual output callback, never at
function entry. Ordinary 64-item Host cases still allocate 378 times afterward
(2,898 requested bytes, zero reallocations), and full calls allocate
2,502/4,121/9,689 times at 1/8/32 Gears. Pre-output reservation/Back allocations
are separately visible. Finite streams/control/numerical/Value retain their
own larger allocator observations; no whole-Host allocation-free claim is made.

Fresh source-matched fixed-kernel probes join all 192 observations across
1/8/32/256 dense/sparse nodes and three processes, including resource Exit0,
raw replay, ready/queue scan counters and pressure refusals. All have zero
ingress/step allocations or reallocations, one successful fixed-store payload
write per accepted one-byte input, and no step fixed-store payload copy.
That counter excludes compiler moves, metadata, Signs and Host transport.
Dense 256-node fixture advance mean is 21.732 microseconds (SD .176), sparse
single-active mean .271 microseconds (SD .031); these are instrumented byte
sinks, not checked Plan/Play rates or per-logical-invocation clocks.

Expanded analysis derives explicit std boundary payload-write extents at the
measured source: successful ingress store plus two owned output buffers
(adapter and report). Fan-out stores an admitted input once; Full attempts do
not write it. For U64 stream1/fanout4 these extents are 24,576/73,728 bytes;
Theremin is 2,420,736 bytes; a 64-Play Value batch is 1,536 bytes. These are
**source-derived boundary extents**, not a hardware total-copy counter.
Intermediate Gear/evaluator/codec, metadata, Sign, compiler and harness copies
are outside this scope. Source file hashes and every derived row are retained;
there is no zero-copy claim.

Fresh production FixedSignLog probes independently join all 480 raw observations
and 15 summaries. A 1,024-record append batch averages 8.262 microseconds;
full transient retention averages 133.181 microseconds, preparation 1.701
microseconds. These include loop/black-box costs, are native scoped batches,
and establish neither whole-scheduler CPU share nor a removable Sign tax.
Normal Plan/Play Signs remain mandatory. Hardware cycles/instructions/branch
misses are unavailable: local `perf` refused under `perf_event_paranoid=4`;
[evidence/std-host-call-index-20261017-perf-counter-refusal.log](evidence/std-host-call-index-20261017-perf-counter-refusal.log)
retains the refusal. Unavailable counters are not zero or estimated from wall.

## Reproduce, inspect and browser boundary

From a clean committed checkout of the measured Source, the single supported
capture entrance is:

```sh
taskset -c 4 cargo +stable xtask prove runtime-performance \
  --output /absolute/new/capture --values 1000000 --repetitions 3 \
  --decisions 1000 --samples 1000 --hosted-pipeline true
```

Choose an allowed CPU locally and repeat in three distinct output directories.
The command builds optimized native code, runs release conformance, verifies
custody, then captures immutable Source/Host/Plan/Play/Sign/raw/report artifacts.
Record actual live worker identities/executable hashes when comparing independent
processes. [runtime-5395-evidence.md](runtime-5395-evidence.md) lists retained
archives, verification commands and source/proof digests. Development proof
helpers operate on these immutable captures; no special product Back or second
scheduler exists. Root verifier checks every distribution against raw data.
Ten rehashed corruptions (control input/output, numerical source/error/type,
Value Play/implementation/eligibility and removed matrix families) are refused.
All 25 release conformance tests pass per capture, strict local Clippy and
formatting pass. The initial missing allocator-test mutex failure and routine
Clippy failures remain retained alongside their repairs and passing checks.

The acceptance permits a scoped subsequent browser/WASM gate. Current matched
Source has **no browser/WASM performance result**. The subsequent gate must
bind sealed optimized WASM and source/artifact hashes to the same checked
1/8/32 computation, verify admitted browser Back/temporal eligibility, required
Signs, exact outputs and refusal/pressure behavior, and separate compile/
instantiate/cold preparation from Play. Run three fresh pinned Chromium
processes with one proof worker and zero retry substitution; retain browser/
engine/version, raw latency/CPU where supported, linear-memory/allocator/copy
scope and WASM size. Unsupported counters stay unavailable. No native/WASM
ratio until framing and output custody match. Historical browser attribution
in [kernel-overhead-attribution.md](kernel-overhead-attribution.md) has different
Source/computation and is not substituted for this gate. No physical, stable
release or hosted-public performance claim follows from native local evidence.

This publication follows the operator's explicit direct-dev/no-PR/no-CI workflow;
it supersedes the issue's older generic Candidate/Integration process note.
No workflow was requested, no thresholds were invented, and optimization
eligibility/meaning/checks were preserved. Later dev commits do not relabel
the source of these frozen measurements.
