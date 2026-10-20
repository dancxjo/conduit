# Resource-bound replacement cost facts (#6101)

`conduit_planner::resource_cost` exposes preparation, transfer, residency,
migration and continuity Time observations for one unchanged logical Gear.
This is comparison input for the existing replanning policy, not a policy,
replacement controller, resource grant or automatic Plan installation.

## Exact evidence and limits

`ReplacementCostComponent::bind` keeps a checked Plan, the original
`CostEvidenceSource`, and actual `CostMeasuredDuration` endpoints. The producing
Sign, Host/Boot/offer generation, capability, selected implementation artifact,
execution profile, calibration and clock quality remain separate facts. The
sample completion must equal its source observation instant. Resource stages also
retain the exact ResourceRef workload, including version and actual extent; a
fresh timing cannot silently serve a different published reference or size. Exact physical
Time intervals retain endpoint resolution/uncertainty rather than treating an
unknown or zero-tick observation as a free operation.

`ReplacementCostFacts::admit` requires all five explicit, correctly ordered
stages and distinct producing Signs. It compares verified Plans for the same
Source/checked/expanded Plot and original Gear, ports, configuration, semantic
contract and source span. Each observation must still match the exact selected
Back and Base instance. Every producer needs one unambiguous, currently fresh
Host/calibration context. Missing, stale, future, conflicting or numerically
unrepresentable evidence refuses; there is no unknown-as-zero fallback.

The first concrete residency profile observes `HostedResourceGeneration` through
`CostResourceResidence`. It borrows the actual provider and its original binding,
opaque handle, published reference and resident byte count. A
`CostResidentResource` also borrows the selected placement and actual
`ResourceAdmissionOwner`, requiring its exact live Plan/placement reservation.
An advertisement or capacity estimate alone cannot establish residence.
Unpublished/candidate, lost, expired/unsupported-lifetime, foreign or released
owners refuse distinctly where relevant. Safe exclusive mutation of this local
provider is fenced while borrowed; the trait does not invent synchronization for
other implementations.

This comparison profile requires one unchanged Host/Boot; cross-Host or
cross-Boot replacement refuses before consuming local copy costs.
There are at most 16 Host contexts and 64 resource-residence entries; each selected
Gear's resource list has the same explicit 64-entry ceiling. This profile accepts
published Play-lifetime content without an independent expiry. Scratch, compute,
device, Boot/durable retention and remote-transfer profiles require their own
actual observers and currently refuse instead of inheriting this local proof.
The provider retains one immutable binding clone during preparation for inspection;
that metadata cost is included in measured preparation. No new observation or
allocation is inserted into kernel Play.

The result retains original components, provider/reservation explanation and a
sequential sum of overhead intervals in seconds. Bytes, slot counts and lifetime
contracts stay separate from Time. This sum is not a whole-program stopwatch,
an overlapping-stage model, a future confidence interval, or a net-benefit
verdict. Finite exact arithmetic spends the caller's explicit work budget.
Refusal cannot mutate either Plan, provider, reservation ledger or old outcome.
Producer authentication and effect authority remain the existing independent gates.

## Actual local retained/copy diagnostic

Use the supported development entrance with a new output directory:

```sh
cargo xtask prove runtime-performance --resource-replacement \
  --repetitions 3 --output /tmp/resource-replacement-new
```

The entrance requires clean committed source, bounds repetitions to 3–20, builds
an optimized worker, captures stdout/stderr and checks source custody before and
after execution. The worker records its executable digest, compiler, CPU, complete
Host inventory, both verified Plans, clock identity/quality, phase observations,
allocations, resident references/bindings/handles and actual reservations.

One ordinary checked `frame/source` specimen has two explicitly selected local
Base/arena realizations over the same Source and semantic contract. Both use the
existing 262144-byte published-frame bound and exact same ResourceRef meaning.
The source is prepared once. Alternating trial order compares retaining that
published generation with copying its actual leased bytes into a separately
reserved and prepared target generation. The copy is initialized and read back;
both its SHA-256 and the original owner's subsequent SHA-256 must match the
unchanged input. Reference bytes and resource version stay equal while concrete
arena, handle, selected Back/Base and candidate Plan identities stay distinct.
Every newly prepared target receives a fresh opaque handle, so an old lease
issuance cannot become current again when a copy arena is recreated.

Cold source/Plan construction is recorded separately from provider costs.
Cold origin preparation is recorded once. Each copied case includes cold target
preparation; the retained case reuses the warm original. Every trial separately
records preparation, actual transfer/read, residence inspection, lease handoff/
copy validation, and original-resource continuity validation. Cost admission and
report construction, target release/disposal, and final original release/disposal
are separately measured. All reservations must be released at the end.

The five-stage resource-provider sum excludes the separately reported source/Plan
construction, admission/report and disposal costs. Fixture payload creation, clock-to-Time evidence conversion and artifact
serialization are observer/harness work outside the named operation intervals;
there is no complete-process or hardware-copy accounting claim. Allocator counters
include harness work inside each named phase; they are not RSS. The clock uses
Linux CLOCK_MONOTONIC with reported resolution, not certified physical accuracy.

This is actual local provider/reservation preparation evidence over a named
software Host. It does not execute or replace an installed Body/Play, move a
remote resource, preserve a live model session or certify full Body continuity.
Those contracts remain #6102 and the existing owners. Service forecasts, retained
old Play outcomes, hysteresis/net-benefit policy, global placement and automatic
release remain separate. Parent #6078 retains the full audit and publication work.

## Retained current-source observations

Measured Source: `f94e025592bf89a2f0408dcdf8f2a5cab21b10a3`, on dev base
`72ab3b367c236653dff0b5726939e08de62390b9`. The supported entrance completed
three alternating repetitions per case with clean source custody. All six
records verify against the original artifact hashes, exact reference workloads,
reservations, stage attribution and exact rational interval sums.

Medians in microseconds from this one process:

| Named operation | Retained original | Prepared local copy |
| --- | ---: | ---: |
| Target preparation | 0.083 | 4.223 |
| Lease/read or actual transfer | 0.135 | 9.029 |
| Residence inspection | 0.088 | 0.109 |
| Handoff and readback SHA-256 validation | 120.769 | 120.281 |
| Original resource continuity validation | 124.218 | 121.535 |
| Cost admission and report construction, separately | 390.985 | 475.427 |
| Target release/disposal, separately | 0.058 | 1.266 |

Retained resource transfer copies zero payload bytes; the target initialization
copies 262144 bytes. SHA-256 validation dominates the named handoff/continuity
stages in both cases. The cost collector/report is more expensive than the
resource copy itself, and is reported separately rather than erased. These are
protocol-specific descriptive observations with uncontrolled affinity/frequency
and background load, not a product speedup or a calibrated policy threshold.
Only this committed-source capture is timing evidence; uncommitted development
probes were functional diagnostics and are excluded.

The [complete original capture](../audits/evidence/resource-replacement-cost-20261020/capture.tar.gz)
has SHA-256 `15b8e43512cf7c99fd971aff62a62d9284fb9086c7df4f435ad9d3bee9bf7664`.
Its [derived summary](../audits/evidence/resource-replacement-cost-20261020/summary.json)
comes from `python3 proof/performance/verify-resource-replacement-cost.py CAPTURE`.
The [negative-control driver](../audits/evidence/resource-replacement-cost-20261020/check-negatives.py)
takes `VERIFIER_PATH CAPTURE_PATH`; six rehashed mutations refuse incorrect sums,
changed references, expired observations, failed source custody, reused concrete
handles and missing reservations. Original captures remain read-only.

At the measured source, all 12 focused resource tests passed, including allocation-
free publication, unchanged real kernel frame behavior, stale copy-lease refusal,
replacement-Boot refusal, exact Back/artifact and resource-extent fencing, missing
stage/context/reservation and arithmetic-budget refusal. Scoped core/Planner
library and Std library/test/example Clippy passed with warnings denied.
[Logs and verifier results](../audits/evidence/resource-replacement-cost-20261020/)
retain their actual command scope; this is not full-workspace or native acceptance.

CI-only upstream changes through `e834020b7e5cf30b0dd02cc1cd98cff0039cc01a` were
merged after the capture, preserving its measured commit as an ancestor. The
core/Planner/Std/xtask and verifier files compare unchanged against the measured
source. This closeout is pushed directly to dev under the user's explicit
instruction; no new Candidate/Integration/main publication is claimed. #6078
retains shared release and full-audit completion.
