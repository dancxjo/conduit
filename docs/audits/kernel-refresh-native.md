# Current native kernel replay and timing refresh

Local evidence for #5231 and #5395, measured from clean source
`88187d381835b40585af635e0f7f8f964e107a7f`. The existing instrumented production
FixedScheduler harness ran three independent optimized processes, each with
eight trials of sparse and dense 1/8/32-node fixtures. No runtime algorithm,
required Sign, or numeric meaning was changed for measurement.

The archived reference is `33f01225da164644e7545303f8d05facd2314094`. Its CPU is
an i7-12700; the current host reports an i9-12900H. The compiler (Rust 1.98.1),
profile, architecture, build command, and reported affinity agree, but hardware
differs. **No timing ratio or speedup comparison is permitted.** The verifier
caught this mismatch before publication and emits null ratios. Absolute current
measurements below are descriptive shared-host observations, not thresholds.

Across 144 matched cases, complete Sign/remote lifecycle text, received values,
readiness, live storage, and terminal outcome match exactly. Allocation and copy
counters match: no ingress/Step allocation or reallocation, and no Step
value-store writes. Successful ingress writes still copy one byte per ready
input; other copies remain unmeasured. Empty pending-table probes in readiness
and context fell from two per ready input to zero. Other recorded work matches.

| Admitted nodes | Pattern | Mean fixture advance (ns) | Sample SD of three process means (ns) | Mean ns per received value |
|---:|---|---:|---:|---:|
| 1 | Dense | 869 | 9 | 869 |
| 1 | Sparse | 259 | 36 | 259 |
| 8 | Dense | 1,749 | 113 | 219 |
| 8 | Sparse | 346 | 97 | 346 |
| 32 | Dense | 4,402 | 326 | 138 |
| 32 | Sparse | 466 | 78 | 466 |

The measured interval advances the already-prepared fixture through consumption
and return to Idle, including counters, assertions, and harness work; it is not
one logical Step or a sustained throughput measurement. Sparse fixtures have
one useful received value; dense fixtures have one per admitted node. Trial
order/cache effects are visible (including the one-node dense case); do not
interpret this table as a scaling law. Raw samples and process CPU/RSS records
are retained. Instrumented object extents increased by 112/168/360 bytes at
1/8/32 nodes relative to the archived source; these include added test counters
and are not production footprint evidence.

These fixtures contain no Host Calls and no quiescence tracks, so they cannot
measure the recent Host-slot or routed-quiescence improvements. Whole checked
Plot/Plan/std Back arithmetic, large graphs, continuous streaming, DSP, browser,
and physical evidence remain separate acceptance work. Both parent issues remain
open. This refresh preserves replay truth while exposing measurement limits.

## Reproduction

Run from a clean committed checkout, with a fresh output directory:

```sh
python3 proof/performance/kernel-overhead.py --output /tmp/kernel-refresh --runs 3
```

Extract `evidence/quiescence-absence-native-raw.tar.gz` and
`evidence/kernel-refresh-native-raw.tar.gz` outside the checkout, then run:

```sh
python3 proof/performance/verify-kernel-refresh.py \
  --before /tmp/reference/quiescence-after-capture-01 \
  --after /tmp/current/kernel-refresh-88187d381 \
  --output /tmp/kernel-refresh-verification.json
```

The verifier checks all 36 artifact hashes, exact case coverage and identities,
successful process exits, compiler/profile custody, full replay parity, allocation
and copy parity, and descriptive work/timing summaries. It rejects altered raw
bytes and duplicate/missing cases even when the latter's artifact hash is updated;
both negative checks were exercised locally. Generated results are retained in
`evidence/kernel-refresh-native.json`. No CI was dispatched or awaited.
