# Cold and warm production pipeline phase report

For #5536, 24 optimized native rows at clean Source
e60af7aba2fe1cf95ca85eab2594ab012c35022a separate elapsed wall and Linux thread
CPU nanoseconds across eight trials of 1/8/32 logical Gears. Every Gear selects
the genuine std reference expression Back. Checked Source, checked/expanded
identities, Plan, both actual Play IDs and selected implementations/artifacts
remain in the raw JSON. The finite U64 pipeline doubles input 7 at each Gear;
a prepared multiplication expression has the same final mathematical result,
with different coordination/evidence work explicitly retained.

| Gears | Cold installed prep µs | Cold Play µs | Warm installed prep µs | Warm Play µs | Warm whole Host run µs |
|---|---:|---:|---:|---:|---:|
| 1 | 79.41 | 11.64 | 52.86 | 6.55 | 210.35 |
| 8 | 150.77 | 36.44 | 128.54 | 25.15 | 349.41 |
| 32 | 458.13 | 131.36 | 428.88 | 125.89 | 883.16 |

These are eight-row wall medians, not realtime guarantees. Corresponding warm
Play thread-CPU medians are 6.60/25.21/125.94 µs. CPU capture precedes wall
capture at each stamp, so very short intervals can show slight cross-clock
boundary skew. Full raw wall/CPU pairs, min/max, standard deviation, compiler,
CPU/topology, OS and clock resolution are retained. The process is unpinned
under concurrent development work; no frequency or affinity calibration is
claimed and no timing threshold gates correctness.

Cold phases include parse, check, expansion, current Host/offer setup and
planning. Checking medians are about 5.1–5.2 ms; Host/offer setup about
9.8–10.0 ms. Expansion and planning grow with the actual graph. Compilation
and deterministic fixture-text construction are outside all timed phases.
Reference Source construction and evaluator preparation are separately labeled.
The prepared one-operation multiplication evaluator takes about 6.75 ms for
100000 validated calls (about 67.5 ns/call); it does not perform 32 Gear
dispatches or their mandatory Signs.

Cold and warm real Plays are both measured, with normalized Sign event kinds
matching and exact outputs verified. Warm follows one measured, verified cold
Play; installed Back preparation repeats for every Play. Whole Host time
includes reporting/reservation work outside the installed kernel markers, so
these costs are visible rather than subtracted. All capacity-one, eight-byte
Value/Cord guards and mandatory Host Call/Sign costs remain in the actual
kernel. Samples retain equal cold/warm Sign counts for each shape.

This is native single-thread production-kernel and prepared-reference evidence,
not a mock scheduler, physical parallelism proof, generalized compiler claim or
cross-target result. The fixture depends on the optional timing markers from
#5542. Strict benchmark Clippy and exact 1/8/32-Gear cold/warm conformance pass;
the ignored release capture passes and all 24 rows independently validate.
Parent #5395 retains its allocation/copy/evidence decomposition and broader
reproducible reporting gates.

## Packed-kernel refresh
The same two release tests passed after refreshing onto dev
`7d60164e59d06f4bd35bb08883e17e3ad9eca6e6`, at clean measurement Source
`b1de3b6ef1ca651576144db2390c7ae776d95c08`. This local source includes the
phase-marker dependency from PR #5847; it is not a claim that this dependency
or the refresh has already merged. All 24 rows retained exact output/provider,
Plan/Play, fixed-capacity and cold/warm Sign checks. The original capture above
is preserved; the separate refresh receipt is
`evidence/cold-pipeline-packed-phase-samples.json`.
| Gears | Cold prep µs | Cold Play µs | Warm prep µs | Warm Play µs | Warm whole Host run µs |
|---|---:|---:|---:|---:|---:|
| 1 | 78.36 | 10.30 | 52.89 | 6.62 | 211.81 |
| 8 | 184.37 | 30.18 | 128.39 | 24.19 | 340.21 |
| 32 | 609.87 | 121.96 | 443.92 | 115.97 | 891.82 |

These remain uncontrolled native wall medians; raw thread-CPU samples and
dispersion are separate. They do not establish a causal scheduler speedup.
