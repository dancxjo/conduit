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

The directory must be new. Compilation precedes measurement. The driver pins the
repository toolchain and optimized profile, verifies exact source and executable
custody, launches three independent test processes, retains stdout/stderr and GNU
time CPU/RSS records, and hashes all artifacts. Each process contributes 48 raw
cases. Summaries retain each process's mean across eight trials, plus descriptive
between-process sample variation. No confidence interval, timer subtraction,
uncontrolled-host speedup, production kernel object size, or threshold is claimed.
Observed wall timings include diagnostic counter and harness work.

The native test allocator/scan seams are not installed in the browser/WASM
product. Browser attribution therefore remains an explicit unsupported proof
boundary; the browser can run the product but does not expose these measurements.
This does not imply a browser performance result or close parent #5395, whose
full Host pipeline, continuous-stream, DSP, payload/fan-out and numerical-profile
acceptance remains separate.

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
