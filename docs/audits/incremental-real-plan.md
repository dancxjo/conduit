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

Validation at `c9e599ef2c4ad85fc1d51c75cda422bbf6a05d71`, based on accepted
`df66968dec9cdf6eddb813857bf617fd4c3326dd`: 38 tests pass across six focused targets covering
incremental cache, real Plans, global hard admission, bounded global search,
cost selection and capacity-aware cost planning. Strict focused Clippy and
pinned workspace formatting pass. A dedicated build cache and unchanged-source
mtime refresh exclude relocated-worktree cache reuse.

Proof class: native checked Source and the real ordinary planner with an explicit
cache test adapter, modeled offers and exact cost-source identities. No live
provider, cryptographic producer authentication, installed runtime replan service,
physical transport, execution benefit or parent #5230 completion is claimed.
