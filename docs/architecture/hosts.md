# Hosts and execution

A Host is a software environment that can offer and execute bounded Conduit
work. A machine can contain several Hosts; a Host can be headless. The authored
Form describes meaning, while the selected Host supplies its implementation.
Read the [canon](../conduit-canon.md) for the architectural rules and
[STATUS.md](../../STATUS.md) for current implementation and proof limits.

## From source to an effect

```text
canonical .conduit source
    -> checked Form and checked Faces
    -> expanded Form, including selected reusable Backs
    -> Plan over current Host offers, resources, authority and Lines
    -> prepared Host-assigned fragments and numeric kernel tables
    -> Play through the shared execution kernel
    -> admitted Host operations and correlated Signs
    -> Presentation and a Host-specific Manifestation
```

Each stage has its own identity. Source spelling is not checked meaning;
checked meaning is not placement; a Plan is not an active Play; a rendered row
is not execution evidence. A refusal at one stage does not establish success
at a later stage.

## What a Host owns

A Host owns its current Boot, implementation initialization, finite resource
availability, capability offers, local preparation, and platform effects.
It advertises only what its current composition and resource truth support.
`HostId` identifies the Host; a fresh `BootId` identifies each incarnation.
An old Boot's Plan bindings do not silently become valid after a restart.

An installed implementation, a current offer, a selected capability, a reserved
resource, and an active instance are separate states. The planner selects among
[equal checked Faces](functional-compatibility.md), then seals exact
implementation, artifact, Host, Boot, resource, authority and bound facts.
A familiar Kind name alone does not establish compatibility or availability.

Planning is an [optional Host capability](portable-planner-capability.md).
A Host without a planner can execute its assigned fragment. Planning produces
an immutable Plan without starting it or granting permission to perform effects.

## One kernel, explicit effects

[Plan lowering](plan-kernel-lowering.md) turns rich identities into finite
numeric tables before Play starts. Hosted storage may allocate during
preparation; the admitted execution path cannot grow its queues or hide work
in an unbounded callback stream. Unsupported profiles refuse before execution.

The [execution kernel](../../architecture/kernel/) owns scheduling, typed Port
traffic, atomic fan-out, pressure, operation correlation, closure, cancellation,
and terminal evidence. Platform adapters perform admitted Host operations and
return their exact completions. They do not become a second scheduler.

A protected effect also needs current authority at its trusted provider.
[Cooperative authority checks and hostile-code confinement](implementation-confinement.md)
are different guarantees. A Plan or serialized grant identity is not by itself
an isolation mechanism.

## Bodies, Forms, and connections

A [Body](body-lifecycle-waists.md) is a durable logical computer with a bounded
workset of zero, one, or many Forms. A Part is an admitted membership relationship;
a Host and Boot describe current machinery. Membership, reachability, capability,
and authority do not imply one another.

One Body-wide Plan covers the workset; one active Play realizes it during a Wake.
Workload replacement follows the bounded lifecycle transaction, retaining the
Body and exact evidence. Body continuity is not represented by recursively
pretending that a set of Hosts is another Host. Reusable composition belongs to
Forms and their Faces/Backs.

A Cord is a typed semantic connection. A remote Cord uses an exact planned
[Line](route-candidates.md), with a separate [session and attachment](session-route-attachment.md).
New availability observations cannot add an unplanned route or invent retry,
reconnect, membership, or authority. [Replacement planning](topology-planning-play-control-loop.md)
creates fresh immutable realization truth.

## Implementation map

| Responsibility | Source |
| --- | --- |
| Shared architecture vocabulary | [`architecture/core`](../../architecture/core/) |
| Assigned-Plan schema and validation | [`architecture/assigned-plan`](../../architecture/assigned-plan/) |
| Source, checking and expansion | [`architecture/form`](../../architecture/form/) |
| Planning and admission | [`architecture/planner`](../../architecture/planner/) |
| Numeric lowering | [`architecture/plan-lowering`](../../architecture/plan-lowering/) |
| Shared execution kernel | [`architecture/kernel`](../../architecture/kernel/) |
| Body membership and lifecycle | [`architecture/body`](../../architecture/body/) |
| Read-only runtime projection | [`architecture/observatory`](../../architecture/observatory/) |
| Portable hosted implementation | [`targets/std`](../../targets/std/) |
| Browser Host and runtime | [`targets/browser`](../../targets/browser/) |
| Native ConduitOS machinery | [`targets/conduitos`](../../targets/conduitos/) |
| RP2040 fabrication and firmware | [`targets/rp2040`](../../targets/rp2040/) |

[Fabrication](../host-fabrication.md) selects machinery through
PROFILE → BUILD → IMAGE. Building an image does not create a live Host, a Boot,
a Body membership, or an offer. Runtime observations establish those facts.

The early [portable-Host proposal](portable-hosts.md) and
[CHS-0 specification](host-specification.md) are retained as historical rationale.
Their composite-Host model and milestone requirements are not current API or
contributor requirements.
