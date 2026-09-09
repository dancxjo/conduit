# Portable bounded navigation

**Status:** development contract for [issue #2232](https://github.com/dancxjo/conduit/issues/2232)

**Canonical Form:** [`forms/bounded-navigation/main.conduit`](../../forms/bounded-navigation/main.conduit)

Portable navigation is the semantic waist between a spatial goal and bounded
body-motion intent. It lets a human, model, or another Form propose where a
Body should go without giving that proposer local-control, actuator, or motor
authority.

```text
current pose + goal + finite traversability + explicit current time
                    |
                    v
            route decision
                    |
                    v
               trajectory
                    |
                    v
             local control
                    |
                    v
       portable body-motion request
                    |
                    v
     authority + mandatory local safety
                    |
                    v
             exact actuator Base
```

## Portable contracts

The first bounded profile uses a named spatial frame and a 4 by 4
traversability grid. It is deliberately smaller than a mapping or SLAM system.
The following values remain mechanically distinct:

- `NavigationGoal` identifies an exact target or hold request, its arrival
  tolerance, and its finite lifetime.
- `NavigationTime` names the clock and current instant used to evaluate every
  freshness and expiry decision; implementations cannot consult ambient time.
- `NavigationPose` carries current position and heading, source and sample
  identity, uncertainty, and freshness relative to the navigation decision.
- `NavigationTraversability4x4` is one finite observation in the same frame. It
  is evidence from a named source, not universal map truth.
- `NavigationRouteDecision` is either a bounded route or one exact refusal. A
  route retains its goal and exact planning-input identity; the immutable
  Conduit Plan and Patchbay retain the selected route-implementation identity.
- `NavigationTrajectory` time-parameterizes a route under finite horizon and
  kinematic bounds. An abstract route remains distinct from this temporal
  intent.
- `NavigationControl` is either a portable `RoboticsMotionRequest` or one exact
  controller refusal. It is not an actuator result.

All distance, angle, duration, velocity, and uncertainty fields use the
repository's typed quantity contracts. Frames must match exactly. Point,
segment, grid, identity, and encoded-value limits are finite. An unknown or
stale pose cannot be treated as a current pose merely because coordinates were
observed earlier.

Across typed navigation outcomes and lower Plan/actuator evidence, these
conditions remain different:

- invalid goal;
- stale or unavailable pose;
- unavailable traversability evidence;
- no route in the admitted finite environment;
- unavailable local controller, which prevents an exact Plan rather than
  becoming a fabricated control value;
- authority refusal below portable control; and
- physical-safety inhibition at the actuator realization.

The last two are lower-layer outcomes. Navigation must not translate either
one into `no route`, retry it invisibly, or report that a motion request was
physically realized.

## Ordinary Form work, not a Conduit Plan

The canonical `bounded-navigation` Form composes three ordinary Kinds:

```text
navigation/route-grid4
  pose + goal + traversability + time -> decision(route | refusal)

navigation/time-parameterize
  selected route + time -> trajectory

navigation/local-control
  pose + trajectory + time -> control(motion request | refusal)
```

Variant selection with `unmatched=drop` prevents a refusal from being fed into
the next stage. The same decision and control values remain visible at the
Form boundary. Every successful stage is an explicit typed Cord; there is no
callback loop hidden in Pete or Patchbay.

Each invocation computes one bounded decision and control result. A separately
admitted receding-horizon composition may invoke this work again for newer
inputs during one Play. That does not mutate the immutable **Conduit Plan**,
which owns the exact realization of the authored Form on current Hosts, Bases,
resources, and authority. Navigation code must not use `PlanId` for route
identity. Changing the selected route or controller implementation leaves the
authored goal and the Form's portable Face unchanged.

## Authority and Create safety boundary

The Form ends at `RoboticsMotionRequest`. Producing that typed value grants no
motor authority and performs no physical effect. A model proposal, human
interaction, valid route, Body membership, or reachable Host cannot fabricate
the capability required by the selected actuator realization.

For Pete's Create realization, the boundary below navigation remains the
[#1521 Brainstem migration contract](pete-brainstem-migration.md). The selected
physical drive offer must pass through `LocalCreateDriveSafety`, which checks
current authority, TTL, safety generation, hazards, provider state, and the
admitted safety profile before lowering to Create OI wheel commands. It owns
finite stop work for expiry, authority loss, hazard, cancellation, and provider
failure. That boundary is not an author-wirable Gear, and this navigation Form
cannot route around it.

Deterministic route and controller evidence may use a non-actuating fixture.
Such evidence proves portable values, refusals, bounds, and control behavior;
it does not prove a Create moved or stopped in the physical world.

## Generic Patchbay observation

Navigation needs no private debugger. The canonical Form exposes each semantic
stage through ordinary Gears, typed Ports, and Cords. Existing bounded Patchbay
Watches can attach to those exact subjects, while the causal timeline follows
the retained parent sequence through goal, route decision, trajectory, control,
motion request, and the lower actuator Signs.

Patchbay presentation remains a read-only projection. It may render the goal,
pose age and frame, route, selected trajectory segment, control result,
authority refusal, safety inhibition, and exact actuator outcome only from
current typed values and retained Signs. Missing telemetry must remain visible;
the UI cannot infer a route, freshness, authority, safety clearance, or physical
success.

## Proof boundary

The deterministic proof uses one exact 4 by 4 fixture to establish a finite
route, trajectory, and expiring body-motion request, plus distinct invalid,
stale, unavailable, no-route, and controller-refusal cases. A production-kernel
oracle must correlate its exact Plan and Play rather than treating direct Rust
evaluation as Form execution.

Closing #2232 additionally requires a tiny attended Pete movement through the
same `RoboticsMotionRequest` and #1521 safety/authority boundary. The receipt
must identify the reviewed start and goal or bounded segment and independently
verify the terminal safe stop. A deterministic fixture, browser execution,
emulator, attached-but-idle device, prior recording, or generated screenshot
cannot substitute for that physical evidence.
