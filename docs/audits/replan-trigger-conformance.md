# Shared-pool pressure, substitution and replanning triggers

Development proof for #5567, through the existing supported entrance:

```sh
cargo xtask prove local-model-pool
```

The additional `replan_triggers` corpus uses the accepted typed provider model
and the ordinary canonical Planner, fresh finite prepared costs, current
provider observation identities and production `HostedSharedPool` runtime.
It does not add a policy engine or mutate a Plan.

A queue-cost sequence of 100, 110, 100, 90 and 110 exercises a ±10% wobble.
Every operation selects inside the existing sealed envelope. Selection evidence
retains the original PlanId and cannot become an exhaustion replan request.
When the initially selected provider becomes unavailable, an eligible alternate
uses that same PlanId. The complete Plan remains equal to its original value.
Queue cost changes are observations for new-operation selection; they do not
move an in-flight operation or rewrite its admitted identity.

Occupied members produce a distinct `CapacityRefused` result while provider
observations remain Ready. Capacity pressure cannot produce replacement-Plan
control-loop events. Losing every admitted provider instead produces actual
`EnvelopeExhausted` evidence through the runtime. Its existing validated event
conversion emits `PlayBecameUnsatisfied` with the prior Plan identity, followed
by `PlanningRequested` from the admitted fragment owner.

These are typed deterministic fixture observations and retained Sign identities,
not cryptographic signature verification, hostile-provider confinement or
physical failure measurements. No fixture printer performs an effect. The same
shared-pool policy and evidence apply across workload domains.

This slice classifies causes and tests the existing no-replan boundary. It does
not implement a profitable global replan, a hysteresis/cost-benefit threshold,
or an incremental planner cache. Those remain #5568/#5571 and parent #5233.
A PlanningRequested event requests planning; it is not a successful new Plan.
