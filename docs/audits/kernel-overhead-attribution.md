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
