# Incremental/cold ordinary Plan conformance

For #5555, the existing finite `IncrementalPlanner` retains results of actual
`plan_selected_realizations_with_costs` evaluations. Both candidates are ordinary
single-Gear Plans for the same checked pulse Source, using distinct modeled Host
advertisements, current bounded resource observations, exact cost-source identities,
calibration and dimensioned point forecasts. No production planner/cache/replan
policy changes. The test adapter is explicit; this does not claim a new installed
global incremental planner, automatic migration or product replan service.

Each candidate declares its exact advertisement, capacity, authority and forecast
content dependencies. SHA-256 content identities change while generation stays
at one, exercising content freshness independently of generation counters. A
cost change reverses the winner; loss of admitted capacity or authority rejects
the affected candidate. Only that candidate is reevaluated. Both candidates
refuse when neither is eligible, then recover when the facts recover. Cold and
incremental evaluation dispositions, selected Plan identity and cost agree at
each step. Every admitted Plan passes ordinary verification. A changed placement
has a distinct Plan identity; old complete Plan evidence remains equal and
verifiable. Restoring identical facts may correctly recover the original identity.

The fixture caps observations at existing pool availability; it never creates
capacity. The two retained cache slots bound the candidate envelope. The
`evaluation_work_units` field counts one modeled evaluation call, not measured
CPU/time or a performance claim. Costs are the same finite millisecond values
bound through the existing cost evidence/profile path; no opaque host score.

Validation Source: `ae7a0eb0db2e88691669f540a1945a29094ae1e7`. The six focused existing/new test
targets pass 38 tests, covering incremental cache, real Plans, global hard
admission, bounded global search, cost selection and capacity-aware cost planning.
Strict focused Clippy passes with warnings denied. Origin's two 4907
incremental branches have no planner commits missing from dev; their unique
non-planner histories remain retained. This proof reuses the accepted owner.

Proof class: native checked Source and real ordinary planner with modeled offers
and cost-source identities. No live provider, cryptographic producer-authentication,
physical transport, execution benefit or parent #5230 completion is claimed.

Fresh current-development validation at `275b83e100c7fc27c5c959bc3a0dcb01e891d6e0`, after
`ac97b99fd9016e5879f9822432fe945ee3c500f0`, passes all 38 tests across six targets, strict
focused Clippy and pinned workspace formatting. Compiler and semantic inputs
were refreshed unchanged to prevent shared-target reuse from another checkout.
The real-Plan conformance file remains byte-identical to the earlier measured
source; this adds no production cache, scheduler or provider-authentication claim.
