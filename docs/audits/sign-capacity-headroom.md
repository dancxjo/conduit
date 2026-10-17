# Admit Sign reservations without scanning unused history

`FixedSignLog` and `HostedSignLog` previously counted non-transient events for
every `ensure_capacity` call, including the entire allocated event array's
empty tail. The checked parser proof admits 60,000 local and remote Sign slots
per graph, so this is a concrete candidate contributor to repeated parser
 execution cost. It is not yet an attribution of the whole parser test.

Admission now returns immediately when both the actual occupied item count and
used-byte count leave physical headroom for the reservation. Checked addition
prevents counter overflow. If either budget is tight, the original retained
history rule, error precedence and classification apply. No counters, fields,
allocations, eviction decisions or Sign identities were added or changed.

Classification, capacity admission and gap extension were extracted into the
private `sign_retention` module. The root shrinks; public paths remain intact.
The admission helper is inline so concrete fixed/hosted sinks retain their
optimization context. An initial capture exposed slower pressured walks;
separating the walk did not remove it. Inlining admission at each sink produced
the final observations below. Earlier captures remain diagnostic evidence.

## Production reservation observations

The manual integration probe uses ordinary, non-test kernel library code and
real fixed/hosted sinks. Fixed capacities are 1/8/128/4096; hosted capacities add
60,000. Each covers empty, sparse, half-transient, full-transient and full-sticky
history. Preparation and exact history/occupancy/byte/gap checks occur outside
timing; 256 actual reservations and result stores occur inside each batch.

Each side retains three independent processes, eight trials per case, 1,080 raw
rows and 45 recomputed summaries. Profile: test optimization 1, assertions and
overflow checks enabled, no debug info or incremental compilation. Sources and
executables are pinned in the [receipt](evidence/sign-capacity-native.json).
All non-timing rows agree exactly. The probe also asserts exact history and gap
preservation after each batch; this does not measure total heap allocation.

Batch microseconds, mean ± sample SD of the three process means:

| Sink | Slots | State | Before µs | After µs |
|---|---:|---|---:|---:|
| hosted | 60,000 | Empty | 5130.923 ± 91.443 | 0.464 ± 0.055 |
| hosted | 60,000 | Sparse | 5056.343 ± 51.559 | 0.465 ± 0.039 |
| hosted | 60,000 | FullTransient | 8343.662 ± 88.385 | 6837.507 ± 96.739 |
| hosted | 60,000 | FullSticky | 8340.091 ± 87.478 | 6772.695 ± 54.513 |
| fixed | 1 | FullTransient | 1.048 ± 0.015 | 1.518 ± 0.030 |

Large logs with physical headroom avoid the capacity-dependent walk. The
pressured path still scans retained history; it is not constant time. Results
are not universally faster: the full one-slot fixed sink adds roughly two
nanoseconds per reservation in this capture. A one-slot Sign log is distinct
from a one-Gear workload; no whole-workload ratio is claimed. Inspect every
capacity/state in the receipt, rather than extrapolating from the largest case.
Shared-host load, loop/barrier/result-store work and timer noise are included.
There is no automatic timing threshold.

## Reproduce and verify

Capture the same ignored integration probe at each clean source using
`proof/performance/sign-capacity-cost.py --output /absolute/capture/path`.
These are repository-internal measurement tools, not product entrances. Use an
exclusive Cargo target per worktree/source context. A sequential worktree
switch in a shared target reused an old executable during one diagnostic
experiment; that capture was discarded and the final source rebuilt freshly
in a separate target. The final receipt binds distinct before/after executables.

The [raw archive](evidence/sign-capacity-native-raw.tar.gz) retains build records,
stdout, resource logs, metadata and artifact hashes. After extraction:

```sh
python3 proof/performance/verify-sign-capacity-comparison.py \
  --before /absolute/captures/sign-capacity-before-recheck-capture-01 \
  --after /absolute/captures/sign-capacity-after-inline-admission-capture-01 \
  --output /absolute/new-comparison.json
```

The verifier checks all 26 artifact hashes, raw-row joins, exact case coverage,
ordinary-library profile, non-timing equality and all summaries. It accepts
retained archives without requiring historical Cargo executable paths. Archive
round-trip verification produced the same comparison byte for byte.

## Validation and remaining work

The final source passes 149 kernel tests with four manual observations ignored,
both with the ordinary ready path and the linear reference feature. Strict
kernel all-targets Clippy passes. New tests cover zero history visits with
physical headroom, exact fallback under either pressure budget, differential
retained-history admission across small capacities and counter overflow.

Workspace Clippy and full workspace tests remain required before publication
because this change extracts a module. Actual parser-selection before/after
execution remains necessary to quantify how much of that CI test this removes.
This does not close #5231: its large/sparse graphs, pending-call stress, fairness,
timing profiles, FARGAN and full-Host performance obligations remain open.

## Occupied-prefix query follow-up

Event iteration and remote-identity lookup now restrict their input slices to
`len` and `remote_len` respectively, for both fixed and hosted logs. Storage
remains admitted in advance. Append writes exactly at the occupied extent and
transient eviction compacts that prefix, so empty allocated tail slots carry
no additional history. This also bounds the pressured admission fallback by
occupied history rather than allocated capacity.

A deterministic integration regression failed on the preceding implementation:
an empty 128-slot log's iterator upper bound was 128. It is now zero, and grows
with occupancy. The fixture checks every prefix through saturation and one
transient eviction, including exact event ordering, retention gap, missing-kind
queries and retained/missing remote identities, in both storage profiles.
Current local kernel tests with `alloc` and strict all-target Clippy pass.
This is a traversal-bound result; no new wall-clock speedup is claimed, and the
historical timing table above remains tied to its original source artifacts.
