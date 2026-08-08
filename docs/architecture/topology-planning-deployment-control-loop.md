# Observation, planning, deployment, and execution control loop

This is the checked architecture contract for [R1 #466], replacement planning
[#495], and same-Plan route selection [#496]. It uses current Conduit vocabulary
and the functional-compatibility rule from #522.

```text
boot-scoped observations
  host advertisements, equal checked faces, resources, authority, links
        |
        v
planning request + explicit requirements/policy
        |
        v
new immutable Plan
        |
        v
deployment
  install exact fragments/bindings, issue fresh Plays
        |
        v
execution
        |
        v
evidence + new observations
```

These arrows are inspectable transitions, not one `fallback()` operation. A
link provider reports observations. A planner constructs Plans. Deployment
installs or rejects exact Plan fragments. The execution kernel runs an admitted
Play. Evidence reports what occurred. None inherits another responsibility by
convenience.

## Responsibilities and identity

| Stage | Inputs | Durable or semantic output | State that remains outside Plan identity |
| --- | --- | --- | --- |
| Observation | current host boot/generation reports, capability/resource facts, authority facts, link health | evidence identities and exact facts supplied to planning | readiness, utilization, pressure, current selected route, carrier state |
| Planning | checked semantic intent, equal-face candidate realizations, hard requirements, current observations, explicit policy, exact grants | a new immutable Plan with exact host/implementation/artifact/resources/authority/routes/bounds | planner identity, scratch state, rejected candidates, mutable readiness |
| Deployment | one exact Plan and current host/boot/provider state | installed exact fragments/bindings and fresh Play identities, or an explicit refusal/unsatisfied record | adapter handles, sockets, provider queues, current route selection |
| Execution | installed fragment, active Play, admitted inputs/effects | terminal or continuing Play evidence | transient pressure, in-flight work, provider-local identifiers |
| Evidence/observation | exact runtime/provider facts | bounded machine-readable records that may become fresh planning input | inferred causality or authority not stated by the producer |

Plan identity contains facts whose mutation would change the admitted
realization: checked/expanded form identity, host and boot/generation,
implementation and artifact, resource and authority bindings, finite limits,
connection candidate order, and fragment commitments. It does not contain
current link health, selected-route state, utilization, queue pressure, physical
lane assignment, Play identity, or evidence identity.

A Plan is immutable. New topology observations are never patched into it. A
planning operation that chooses a materially different realization produces a
new `PlanId`; calling that mutation, retry, or failover is incorrect.

## Two different recovery branches

### Same-Plan route selection

The active Plan may seal several exact ordered `BoundLink` candidates for one
connection. `RouteMachine` may select another Ready member of that set. The
result is `ControlLoopEvent::RouteSelectionChanged` with the unchanged
`plan_id`, exact connection, previous/selected binding identities, and the
observation evidence. It does not emit planning-success, Plan-supersession, or
deployment-generation evidence.

The selected route must already be in the connection's sealed candidates. A
newly discovered compatible carrier is not eligible until a new Plan admits it.
Changing the active attachment may require finite session reconciliation, but
it does not alter semantic or Plan identity.

### Replacement planning

When no admitted route remains, or another required host/resource/authority fact
is unavailable, deployment may become unsatisfied. Unsatisfaction is an
observable fact, not an automatic command to plan. Authorized software may then
request planning with fresh observations. Planning can refuse or return a new
Plan. Deployment must explicitly supersede the prior Plan, terminate or retain
old Plays according to exact proof, install the replacement fragments/bindings,
and issue fresh Plays.

The minimum successful sequence is therefore:

```text
LinkBecameUnavailable (when a link caused the change)
DeploymentBecameUnsatisfied
PlanningRequested
PlanningSucceeded(prior PlanId, replacement PlanId)
PlanSuperseded(prior PlanId, replacement PlanId)
DeploymentInstalled(replacement PlanId)
```

`PlanningRefused` terminates the planning attempt without inventing a Plan or
deployment. Failure to deploy a successful Plan remains a deployment failure;
it is not retroactively a planning refusal.

## Machine-readable transition vocabulary

`conduit-core::ControlLoopEvent` defines the minimum shared vocabulary:

- `LinkBecameUnavailable`;
- `DeploymentBecameUnsatisfied` with a typed reason;
- `PlanningRequested`;
- `PlanningRefused` with a typed reason;
- `PlanningSucceeded` with distinct prior and replacement Plan IDs;
- `PlanSuperseded` with those exact identities;
- `DeploymentInstalled`;
- `RouteSelectionChanged` within one unchanged Plan.

Every record carries exact evidence identity. Route records validate against the
active Plan and connection's sealed candidates. Replacement records reject Plan
identity reuse. The unsatisfied reasons deliberately exclude queue pressure:
bounded pressure remains ordinary execution state while the current realization
is still valid.

This vocabulary records facts only. It does not authorize a planner, choose
policy, install fragments, switch carriers, or issue lifecycle authority.

## Required design answers

### 1. Unsatisfied versus temporarily pressured

A deployed Plan is unsatisfied only when a fact required to realize it is no
longer available: no admitted route is Ready, a required host/resource is
unavailable, or required authority is unavailable. A full finite queue, a busy
provider, or backpressure with an admitted continuation is pressure. Pressure
does not justify Plan replacement and has no `DeploymentUnsatisfiedReason`.

### 2. Who may request planning

Planning may be requested by any software acting within an explicit local or
delegated authority/policy boundary; no host is a privileged coordinator.
Requesting planning grants no authority to deploy, replace a Plan, terminate a
Play, use a resource, or contact an external subject. Those remain separately
admitted operations. Planner capability availability is not deployment
authority.

### 3. Reused and freshly observed inputs

The checked semantic intent and explicit durable requirements may be reused.
The old Plan is evidence/context, never a mutable template. Host boot and offer
generation, capability implementations, resources/utilization, authority/grant
validity, link health, and applicable policy inputs must be supplied as current
planning inputs. Stale boot-scoped grants and observations fail closed.

### 4. Old Play termination and supersession

Plan supersession and Play termination are different evidence. Deployment first
records which Plan supersedes which. Every old Play then reaches an exact
terminal/cancelled disposition under its existing identity; late completions
remain invalid. Replacement deployment issues fresh Play identities bound to
the replacement Plan. An acknowledgement of a request is not terminal proof.

### 5. Retaining fragments and bindings

Retention is permitted only when the replacement Plan independently seals the
same fragment/binding commitments and the target host, boot, offer generation,
implementation, artifact, resources, authority, ports, bounds, and relevant
session identity all match exactly. Equal checked faces alone are insufficient:
they admit candidates during planning but do not transfer a prior realization,
Play, grant, reservation, or attachment. Without that proof deployment replaces
the state and reports the gap.

### 6. Observatory presentation

Observatory presents an ordered evidence view, not invented causality:

1. old Plan and old Plays;
2. exact changed observation and producer evidence;
3. deployment-unsatisfied record if one was emitted;
4. planning request and refusal or successful replacement Plan;
5. Plan-supersession evidence;
6. deployment installation and fresh Plays;
7. later execution evidence.

Missing records remain visible gaps. Mere timestamp adjacency must not be
rendered as “link loss caused replan,” and a selected-route change must not be
shown as a new Plan generation.

### 7. Selected-route changes

A selected-route change records one unchanged `PlanId`, exact connection ID,
previous and selected admitted binding IDs, and the observation evidence ID.
The selected provider attachment and runtime session state remain outside Plan
identity. If the desired binding is absent from the sealed set, route selection
fails and software may separately report unsatisfaction/request replanning.

## Holographic-host and proof boundaries

Any eligible host with an advertised planner profile may perform the planning
operation from the same portable inputs; a non-planner host remains a complete
execution target. This contract introduces no coordinator service, consensus,
durable membership database, optimizer, or ambient authority.

The focused tests prove event identity validation and same-Plan candidate
membership. They do not implement Wi-Fi, deployment orchestration, live route
failover, physical recovery, or the R1 board proof. Those remain owned by
[#495], [#496], and [#504].

[R1 #466]: https://github.com/dancxjo/conduit/issues/466
[#495]: https://github.com/dancxjo/conduit/issues/495
[#496]: https://github.com/dancxjo/conduit/issues/496
[#504]: https://github.com/dancxjo/conduit/issues/504
