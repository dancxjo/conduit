# hosts and execution

A host is a software environment that can offer and execute bounded Conduit
work. A machine can contain several hosts; a host can be headless. The authored
form describes meaning, while the selected host supplies its implementation.
Read the [canon](../conduit-canon.md) for the architectural rules and
[STATUS.md](../../STATUS.md) for current implementation and proof limits.

## From source to an effect

```text
canonical .conduit source
    -> checked form and checked faces
    -> expanded form, including selected reusable backs
    -> plan over current host offers, resources, authority and lines
    -> prepared host-assigned fragments and numeric kernel tables
    -> play through the shared execution kernel
    -> admitted host operations and correlated signs
    -> presentation and a host-specific Manifestation
```

Each stage has its own identity. Source spelling is not checked meaning;
checked meaning is not placement; a plan is not an active play; a rendered row
is not execution evidence. A refusal at one stage does not establish success
at a later stage.

## What a host owns

A host owns its current boot, implementation initialization, finite resource
availability, capability offers, local preparation, and platform effects.
It advertises only what its current composition and resource truth support.
`HostId` identifies the host; a fresh `BootId` identifies each incarnation.
An old boot's plan bindings do not silently become valid after a restart.

An installed implementation, a current offer, a selected capability, a reserved
resource, and an active instance are separate states. The planner selects among
[equal checked faces](functional-compatibility.md), then seals exact
implementation, artifact, host, boot, resource, authority and bound facts.
A familiar kind name alone does not establish compatibility or availability.

Planning is an [optional host capability](portable-planner-capability.md).
A host without a planner can execute its assigned fragment. Planning produces
an immutable plan without starting it or granting permission to perform effects.

## One kernel, explicit effects

[plan lowering](plan-kernel-lowering.md) turns rich identities into finite
numeric tables before play starts. Hosted storage may allocate during
preparation; the admitted execution path cannot grow its queues or hide work
in an unbounded callback stream. Unsupported profiles refuse before execution.

The [execution kernel](../../architecture/kernel/) owns scheduling, typed port
traffic, atomic fan-out, pressure, operation correlation, closure, cancellation,
and terminal evidence. Platform adapters perform admitted host operations and
return their exact completions. They do not become a second scheduler.

A protected effect also needs current authority at its trusted provider.
[Cooperative authority checks and hostile-code confinement](implementation-confinement.md)
are different guarantees. A plan or serialized grant identity is not by itself
an isolation mechanism.

## bodies, forms, and connections

A [body](body-lifecycle-waists.md) is a durable logical computer with a bounded
workset of zero, one, or many forms. A part is an admitted membership relationship;
a host and boot describe current machinery. Membership, reachability, capability,
and authority do not imply one another.

One body-wide plan covers the workset; one active play realizes it during a wake.
Workload replacement follows the bounded lifecycle transaction, retaining the
body and exact evidence. body continuity is not represented by recursively
pretending that a set of hosts is another host. Reusable composition belongs to
forms and their faces/backs.

A cord is a typed semantic connection. A remote cord uses an exact planned
[line](route-candidates.md), with a separate [session and attachment](session-route-attachment.md).
New availability observations cannot add an unplanned route or invent retry,
reconnect, membership, or authority. [Replacement planning](topology-planning-play-control-loop.md)
creates fresh immutable realization truth.

## Implementation map

| Responsibility | Source |
| --- | --- |
| Shared architecture vocabulary | [`architecture/core`](../../architecture/core/) |
| Assigned-plan schema and validation | [`architecture/assigned-plan`](../../architecture/assigned-plan/) |
| Source, checking and expansion | [`architecture/form`](../../architecture/form/) |
| Planning and admission | [`architecture/planner`](../../architecture/planner/) |
| Numeric lowering | [`architecture/plan-lowering`](../../architecture/plan-lowering/) |
| Shared execution kernel | [`architecture/kernel`](../../architecture/kernel/) |
| body membership and lifecycle | [`architecture/body`](../../architecture/body/) |
| Read-only runtime projection | [`architecture/observatory`](../../architecture/observatory/) |
| Portable hosted implementation | [`targets/std`](../../targets/std/) |
| Browser host and runtime | [`targets/browser`](../../targets/browser/) |
| Native ConduitOS machinery | [`targets/conduitos`](../../targets/conduitos/) |
| RP2040 fabrication and firmware | [`targets/rp2040`](../../targets/rp2040/) |

[Fabrication](../host-fabrication.md) selects machinery through
PROFILE → BUILD → IMAGE. Building an image does not create a live host, a boot,
a body membership, or an offer. Runtime observations establish those facts.

The early [portable-host proposal](portable-hosts.md) and
[CHS-0 specification](host-specification.md) are retained as historical rationale.
Their composite-host model and milestone requirements are not current API or
contributor requirements.
