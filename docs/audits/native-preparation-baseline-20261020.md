# Current-source native preparation baseline for #6180

Clean unchanged source `21777e52382cc04d86e85fd7cacfbde0b51365c1` completed the
existing #6090 producer with **three fresh QEMU processes, no retry**, using one
exact image/profile/workload. Exit status was 0. This is the baseline before
new stage attribution or native Source reuse changes, not an optimization claim.

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo xtask make conduitos native-latency-proof --trials 3
```

The family is `2351229-1792552677055956732`; all three runs use ISO SHA-256
`7c3526f402d8c41d65b4c361be4dc9e63f33e9199af3edafb5bfe48977109137` and distinct
Boot identities. The producer retains exact source, Rust and QEMU versions,
profile/build/Host/Boot identities, input/Show custody, stage intervals, typing
and reading observations. The observed image's explicit artifact role is
**architecture-proof-appliance**. It exercises the ordinary graphical journey
with native latency instrumentation; it is not uninstrumented normal-product,
physical-machine, attended-human, stable-release or multicore evidence.

| Fresh run | Full journey wall seconds | Boot → first framebuffer commit, raw ticks | Boot → actual first input ACK, raw ticks | PlotBuild intervals |
| --- | ---: | ---: | ---: | ---: |
| 0 | 92.668463797 | 10,608,116,426 | 11,544,134,694 | 105 |
| 1 | 92.208179997 | 10,583,941,646 | 11,529,667,812 | 105 |
| 2 | 92.167437733 | 10,603,314,906 | 11,526,210,840 | 105 |

Wall time includes QEMU startup, scripted interactions, polling, captures and
teardown. It is not boot-only time. The actual first input ACK has exact current
Face/Show/Host/Boot custody, but its Boot interval includes harness capture and
input dispatch, so it is not time-to-input-readiness. TSC frequency remains
unknown; no tick-to-second conversion is made. Compiler and OS caches are
uncontrolled; a fresh process is not a cold machine.

Each run records 136 timing records and 105 completed PlotBuild intervals.
Their aggregate *nested* tick counts are 155,210,384,617; 153,975,241,352; and
154,292,171,743. These can overlap enclosing WorksetPrepare/KernelPrepare and
must not be added to derive elapsed or exclusive CPU time. For example, run0's
WorksetPrepare interval is 38,432,053,778 ticks and KernelPrepare is
18,317,992,361 ticks; this does not explain which installation, parse, checking,
expansion, lowering or kernel subphase dominates. The existing interval events
publish on completion and still leave long operations silent while running.

All **93 actual screenshots** (31 per fresh process) passed the journey's
original assertions. An independent check verified every PNG length/file SHA-256,
resolution and decoded RGBA SHA-256 against its original manifest; no extra or
unbound PNG was admitted. All frame manifests are complete with no failure.
The instrumented workload deliberately retains its original capture/input
protocol; it does not add the separate normal-product four-Look matrix.
[Full family, raw serial/QMP records, per-trial receipts, PNGs, image metadata,
independent verification and hash ledger](evidence/native-preparation-baseline-20261020/).

## Next required work

#6180 stays open. The current `native_workset/catalog.rs::checked` still installs
five semantic catalog groups, parses, checks and expands static Source for every
call. `resident`, inventory resolution and planning repeat that work. Next add
bounded clocked start/finish/refusal attribution with explicit phase/context,
then reuse the canonical #6087 Source preparation mechanism in a finite installed
inventory and measure identical before/after runs. Preserve root closure checks,
semantic identities, current Host planning and kernel authority; cached Source
is not cached Host authority. No SMP or speedup conclusion is supported yet.
The separate live owner Todo appearance work remains blocked by its recorded
Linux release cross-toolchain prerequisite; #6158/#6055/#6155 remain skipped.
