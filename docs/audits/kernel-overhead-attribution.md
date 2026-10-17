# Fixed-kernel overhead attribution

For #5537, the existing real FixedScheduler scan fixture adds test-only allocator
and value-storage observations. It does not replace scheduling or disable Signs.
The allocator delegates unchanged to System and samples only the current thread;
the value wrapper delegates custody unchanged to FixedValueStore. Both are
reachable only through `cfg(test)` and are absent from product builds.

After ordinary startup has reached Await/Idle, the fixture separately observes
remote ingress and Step-to-Idle. It keeps loaded/sparse 1/8/32-node cases,
round-robin order, full-queue refusal, exact received bytes and retained Signs.
The existing raw rows retain readiness, pending-slot, input-port, Cord and queue
traversals, Step counts, wall duration, Sign entries and Sign byte extent.
New rows add actual allocation/reallocation calls, successful value-store writes,
their explicit copy byte extent, and borrowed value reads. Borrowing is not
counted as copying. Compiler moves, Sign copies and Host transport copies remain
unmeasured; there is no total zero-copy claim. The one-byte sink fixture is not
the whole checked Plot/std Back arithmetic pipeline.

From a committed clean checkout on Linux:

```sh
python3 proof/performance/kernel-overhead.py --output /tmp/conduit-kernel-overhead-01 --runs 3
```

Add `--include-large` to include the existing 256-node loaded/sparse selector
replay fixture (64 cases per process instead of 48). `--source-root PATH` measures
a separate clean historical checkout using the same harness, without modifying
that checkout. Metadata records the fixture sizes, process count, and harness
hash. These options do not alter runtime policy or disable evidence.

The directory must be new. Compilation precedes measurement. The driver pins the
repository toolchain and optimized profile, verifies exact source and executable
custody, launches three independent test processes, retains stdout/stderr and GNU
time CPU/RSS records, and hashes all artifacts. Each process contributes 48 raw
cases. Summaries retain each process's mean across eight trials, plus descriptive
between-process sample variation. No confidence interval, timer subtraction,
uncontrolled-host speedup, production kernel object size, or threshold is claimed.
Observed wall timings include diagnostic counter and harness work.

The native test allocator/scan seams are not installed in the browser/WASM
product. Browser allocator/scan attribution remains an explicit unsupported proof
boundary. The browser observation below measures actual wall time and linear-
memory capacity without pretending those are native allocator/scan counters.
The scoped browser result does not close parent #5395, whose full Host pipeline,
continuous-stream, DSP, payload/fan-out and numerical-profile acceptance remains
separate.

## Retained native capture

Clean measured Source `cf5b84ed96b63c81027e0a4d9fd2fdecea02fb5f` produced
144 joined raw cases in three independent optimized processes. All 18 retained
artifact hashes were independently verified. The [receipt and full raw counter
rows](evidence/kernel-overhead-attribution.json) retain exact hardware, compiler,
profile, executable digest, process CPU/RSS records and process means.

| Active nodes | Ready inputs | Mean Step-to-Idle, microseconds | SD across process means, microseconds |
| --- | --- | ---: | ---: |
| 1 | 1 | 4.566 | 0.778 |
| 1 | 1 | 4.124 | 0.751 |
| 8 | 8 | 31.479 | 5.650 |
| 8 | 1 | 4.029 | 0.887 |
| 32 | 32 | 122.231 | 28.904 |
| 32 | 1 | 4.442 | 1.380 |

Every case observed zero allocation/reallocation calls in both ingress and Step.
Ingress copied exactly one byte per ready input into the fixed value store;
Step borrowed those values and performed no additional value-store write. These
are scoped observations, not whole-Host zero-allocation/zero-copy claims.

The loaded 32-node case performs 64 Steps, 64 queue peeks and 1,056 Cord scans,
retaining 96 Signs occupying 1,920 bytes. Its sparse counterpart performs two
Steps and two queue peeks but still scans 64 Cords, retaining three Signs/60
bytes. Those traversal counts expose a remaining per-Step Cord-search cost;
they do not isolate its CPU share or justify a timing-based failure threshold.
The substantial process variation remains in the report. No optimization was
introduced into the scheduler by this measurement change.

## Ordinary hosted allocation boundary

Clean Source `0861fe53462bec4fe7b99885aee2a54f6c5be778` adds non-destructive
allocator snapshots to the existing ordinary checked pipeline. Three independent
public-entrance captures each passed all eleven optimized conformance tests and
retained nine 1/8/32-Gear trials. All 159 artifact hashes, exact source custody
and 27 actual hosted windows were independently verified. The
[compact hosted receipt](evidence/hosted-overhead-attribution.json) retains
complete measurements, producing Source/Plan/Play/Boot identities, ordered
output hashes, mandatory output-Sign counts and source-linked artifact hashes.

Each first-to-last acknowledgement window covers 63 additional outputs and
observed 378 allocations requesting 2,898 bytes, with no reallocations, across
all nine trials for each Gear count. That is six allocator requests per
additional acknowledgement in this measured Host/collection boundary. It is not
allocation attributed exclusively to the kernel or a whole-Play/Host zero-
allocation claim. The fixed-kernel fixture and this real hosted window are
separate experiments, not a subtraction-based timing decomposition.

| Logical Gears, same32adds/value | Full hosted call mean, ms | SD across process means, ms |
| --- | ---: | ---: |
| 1 | 0.729 | 0.023 |
| 8 | 2.641 | 0.379 |
| 32 | 12.485 | 0.818 |

Full calls include reservation, Back preparation, Play, Sign/report translation
and teardown. The allocation window excludes the earlier preparation and is
bounded by two actual Fore callbacks; missing snapshots refuse rather than
becoming zero. Heap peaks include retained harness metadata. Host transport and
compiler-copy attribution, browser allocator/scan attribution and the larger
parent #5395 full-product/streaming/DSP gates remain explicitly unproven. These
results do not identify a universal native-speed ratio or a causal Sign CPU share.

## Flow output allocation and explicit copies

The hosted measurements above use Flow outputs. On producing Source
`0861fe53462bec4fe7b99885aee2a54f6c5be778`, the installed std Fore path constructs
an owned delivery, then clones it for the acknowledging adapter. Each delivery
owns the six-byte `output` port identity, nine-byte `value/u64` Kind identity,
and eight-byte payload. This source path predicts six allocations requesting
`2 * (6 + 9 + 8) = 46` bytes per output. Every one of the 27 measured windows
matches: 63 outputs, 378 allocations and 2,898 requested bytes. The
[source comparison and arithmetic receipt](evidence/hosted-flow-delivery-attribution.json)
retain the exact evidence identity. The collector
preallocates its 64 delivery slots and moves each delivery into that storage.

This is source-supported attribution that matches both allocator count and byte
extent, not allocator stack sampling. The two explicit payload-buffer copies
account for 16 bytes per output in this path, and the four String clones for
30 identity bytes. These are explicit source-level copies, not total machine
memory traffic or compiler moves. Value-temporal outputs already use a distinct
prepared-slot path; do not extend the Flow observation to that path. The owning
implementation is `targets/std/src/installed_std.rs`; its producing-source blob,
the delivery definition and the identity String representation are unchanged at
accepted development Source `c339a75e8ce5015e3d35b289552176fc6a526790`.

## Direct production Sign record-call probe

The manual `sign_record_cost` integration test separately observes the actual
`FixedSignLog::record` implementation with available capacity and with a full
transient log. The latter includes transient eviction, entry rotation and gap
extension. Both paths preserve exact event sequence, Kind and retained extent;
the full-log probe checks the exact gap endpoints and count. Preparation is
timed separately. No production Sign sink or scheduler behavior is changed.

From a clean committed Linux checkout, reproduce with:

```sh
python3 proof/performance/sign-record-cost.py --output /tmp/conduit-sign-cost-01
```

The directory must be new. Compilation precedes the three independent test
processes. Each process retains 32 trials at capacities 1, 8, 32, 128 and 1,024.
The driver retains source/executable custody, raw rows, GNU time records and
descriptive variation across process means. Timed batches include loop and
black-box barriers; one-record batches are especially timer-sensitive. There is
no timer subtraction, automatic threshold or causal scheduler CPU-share claim.
The probe itself establishes no total compiler-copy or browser result. New
capture results must retain their own producing Source and are not attributed
to the earlier scheduler captures above.

### Retained direct Sign capture

Producing Source `bafa852c973613b779594ab3030dd3a6fa59e91e` yielded
480 raw observations across three independent optimized native processes.
All 13 artifact hashes, each raw-row join and all 15 descriptive summaries were
independently verified. The [receipt](evidence/sign-record-cost.json) retains
compiler/profile, executable identity, host load, raw rows and process means.

| Records per batch / capacity | Append batch, ns (mean ± process-mean SD) | Full transient-retention batch, ns (mean ± process-mean SD) |
| --- | ---: | ---: |
| 1 | 36.3 ± 7.5 | 46.7 ± 11.7 |
| 8 | 151.6 ± 20.9 | 299.1 ± 54.1 |
| 32 | 379.1 ± 60.7 | 854.9 ± 207.7 |
| 128 | 1435.2 ± 211.9 | 5144.4 ± 1404.4 |
| 1024 | 11267.2 ± 1660.5 | 253704.9 ± 77802.9 |

Each batch records as many events as the stated capacity. Full retention records
into an already full transient log; it is not spare-capacity append. At capacity
1,024, each of 1,024 records invokes the production transient eviction and entry
rotation path. This exposes a capacity-dependent retention cost, not a scheduler
CPU fraction or measured optimization. The shared host had concurrent proof and
archive work; process load averages and substantial variation remain visible.
One-record batches are timer-sensitive. No speedup, confidence interval,
automatic threshold or whole-Host conclusion follows from these observations.

## Scoped browser/WASM observation

Three fresh pinned Chromium processes each executed three sequential Plays of
the checked structured-value Window fixture, through actual Source admission,
Plan/browser-kernel execution, a true Boolean manifestation and exact terminal
acknowledgement. The [browser receipt](evidence/browser-overhead-attribution.json)
retains raw wall milliseconds, explicit JS-to-ABI write extent, actual effects,
terminal receipts and linear-memory capacity. Every fetched module digest and
byte extent matched its sealed manifest. Runtime and proof-server Source remain
`513cefa19f81d075587f05ef9a4886ed49e9be1b`; harness Source remains
`39b4a8312` (the receipt retains its full identity).

| Fresh process | Compile/instantiate, ms | First admission-through-terminal, ms | Later two Plays, ms |
| --- | ---: | ---: | --- |
| 1 | 13.900 | 642.900 | 391.800, 390.500 |
| 2 | 12.600 | 643.800 | 389.800, 388.900 |
| 3 | 15.000 | 686.500 | 423.900, 432.300 |

Linear-memory capacity grew from 14,745,600 to 114,163,712 bytes during each
first Play and remained at that capacity for the two later Plays. Capacity is
not live heap, allocator-call count or a leak diagnosis. The measured Source
boundary includes checking/preparation and execution; this is not a steady-Step
measurement. Engine compilation cache and wall-clock precision remain
uncontrolled. This is a different checked computation and representation from
the native U64 arithmetic fixture; no native/WASM speed ratio follows.

After preparing a canonical browser release and installing the pinned browser
proof dependencies, reproduce with:

```sh
CONDUIT_BROWSER_RUNTIME_WASM=/absolute/release/runtime.wasm \
  node proof/browser/node_modules/@playwright/test/cli.js test \
  --config proof/performance/browser-overhead.config.mjs --workers=1 --retries=0
```

The runtime directory must retain `browser-page.json`. A compatible proof static
server with the sealed-runtime selector is required (the repair is #5937). The
local capture used the explicitly named server from Source513 above. Repeat in
three new output directories for independent browser processes. There are no
performance thresholds or automatic browser passes from missing evidence.
