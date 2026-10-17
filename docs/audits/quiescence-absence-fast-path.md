# Skip absent quiescence work

For #5231, preparation records whether the validated active Cord prefix contains
any quiescence connection. Cord specifications remain immutable during the
scheduler lifetime. Both ordinary and boxed preparation derive the same flag;
quiescence-marked inactive padding cannot enable execution work.

When the flag is false, progress/Yield arming and final-idle quiescence emission
return without scanning Cords. Graphs containing a quiescence connection retain
the existing arming, pressure, Sign-capacity and transition path. No allocator,
queue, routing identity, ready policy or terminal policy is added. The existing
quiescence methods were extracted into a private responsibility module before
adding the fast path.

## Before and after

Before Source `d9e579e6a1b313c9108c8a8c9adcc933473934d4` contains the
extraction and actual 1/8/32-capacity characterization, with unchanged scheduling
behavior. After Source `33f01225da164644e7545303f8d05facd2314094` adds the
absence flag and both constructor tests. Producing identities remain separate
from later measurement/report commits.

Each side has 144 cases from three independent optimized native processes.
All 36 artifact hashes were independently verified. All 144 corresponding
replays match exactly for mandatory Signs, remote lifecycle records, outputs,
readiness and retained values. Every other work/allocation counter matches.
The observed ingress/Step allocation and reallocation counts remain zero.

| Actual capacity / active nodes | Ready inputs | Before Cord probes | After Cord probes |
| --- | ---: | ---: | ---: |
| 1 | 1 | 2 | 0 |
| 8 | 8 | 72 | 0 |
| 8 | 1 | 16 | 0 |
| 32 | 32 | 1,056 | 0 |
| 32 | 1 | 64 | 0 |

The before counts are exactly `active_cords * (ready_inputs + 1)`: one complete
arming scan per progressing sink plus the final idle scan. This fixture has
ordinary remote ingress Cords and no quiescence connections. The firm result is
removal of those irrelevant traversals.

| Nodes | Ready | Before Step-to-idle, µs | After Step-to-idle, µs |
| --- | ---: | ---: | ---: |
| 1 | 1 | 3.961 ± 0.663 | 3.179 ± 0.940 |
| 8 | 8 | 25.563 ± 5.217 | 19.572 ± 5.801 |
| 8 | 1 | 3.328 ± 0.599 | 2.552 ± 0.746 |
| 32 | 32 | 97.325 ± 23.630 | 73.377 ± 29.319 |
| 32 | 1 | 3.467 ± 0.729 | 2.469 ± 0.926 |

Values are means and sample standard deviations across three process means,
each covering eight trials. The two equivalent one-node loaded/sparse cases
remain separately retained in the raw receipt. Timings include instrumentation
and harness work on a shared host. Substantial variation and all preparation
observations remain in the [receipt](evidence/quiescence-absence-native.json).
These observations establish no calibrated speedup, CPU-share decomposition,
confidence interval or automatic latency threshold.

The remaining fixture work includes two Steps per ready input, ready/pending
checks, input/queue access and mandatory Signs. Removing quiescence traversal
does not remove these costs. The hosted Flow delivery allocation path identified
by #5537 is also separate.

## Production object footprint

The replay's object size includes unit-test counters, so a separate integration
probe links the ordinary kernel library. Cargo artifact records verify that the
kernel library was compiled with `profile.test = false`. The identical probe,
compiler, release profile and production `HostCallBack` driver are used on both
sides, with one-byte value slots, 128 Sign slots and explicit capacities.

| Node/Cord/pending-request capacity | Before bytes | After bytes | Delta |
| --- | ---: | ---: | ---: |
| 1 | 6,600 | 6,600 | 0 |
| 8 | 11,096 | 11,096 | 0 |
| 32 | 26,504 | 26,504 | 0 |

The flag fits existing layout space in these measured native instantiations.
This is not a guarantee for every generic type or target ABI, nor a heap-usage
measurement. The [footprint receipt](evidence/quiescence-production-footprint.json)
retains both producing Sources, all ten verified artifact hashes, compiler and
profile. The before checkpoint is preserved at
`checkpoint/quiescence-before-footprint-20261017`.

## Reproduction and remaining gates

Use isolated clean checkouts of each native producing Source with the existing
`proof/performance/kernel-overhead.py --output <new-directory> --runs 3` driver.
The [raw archive](evidence/quiescence-absence-native-raw.tar.gz) retains both
complete capture directories. After extracting it into a new directory, compare:

```sh
python3 proof/performance/verify-quiescence-comparison.py \
  --before /absolute/captures/quiescence-before-capture-01 \
  --after /absolute/captures/quiescence-after-capture-01 \
  --output /absolute/new-comparison.json
```

For footprint reproduction, use the receipt's exact Source and run the ignored
`scheduler_footprint` integration test with the retained release/profile flags.
The probe and its complete build/run records are retained by the footprint
[raw capture archive](evidence/quiescence-production-footprint-raw.tar.gz); it
measures the ordinary library, not its unit-test executable.

Validation completed: 143 pre-change kernel tests; 145 after-change tests plus
the linear-ready reference suite; strict all-target Clippy; exact ordinary and
boxed active-prefix tests; existing quiescence rearming and terminal/cancellation
exclusions; independent native/footprint comparisons. The new footprint probe
also passed strict focused Clippy. Full workspace Clippy/tests required for the
module extraction remain pending and must finish before publication.

This slice does not close #5231. Its original 10,000-node, pending-call stress,
full transition/fairness, FARGAN/end-to-end and timing-profile acceptance remains.
No faster quiescence-containing graph, physical preemption, parallelism or
universal realtime assurance is claimed.
