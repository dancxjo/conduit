# hosts and execution

A host is a software environment that can offer and execute bounded Conduit
work. A machine can contain several hosts; a host can be headless. The authored
plot describes meaning, while the selected host supplies its implementation.
Read the [canon](../conduit-canon.md) for the architectural rules and
[STATUS.md](../../STATUS.md) for current implementation and proof limits.

## From source to an effect

```text
canonical .conduit source
    -> checked plot and checked fores
    -> expanded plot, including selected reusable backs
    -> plan over current host offers, resources, authority and lines
    -> prepared host-assigned fragments and numeric kernel tables
    -> play through the shared execution kernel
    -> admitted Host Calls and correlated signs
    -> face and a host-specific show
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
[equal checked fores](functional-compatibility.md), then seals exact
implementation, artifact, host, boot, resource, authority and bound facts.
A familiar kind name alone does not establish compatibility or availability.

Planning is an [optional host capability](portable-planner-capability.md).
A host without a planner can execute its assigned fragment. Planning produces
an immutable plan without starting it or granting permission to perform effects.

## Selecting external preparation in Host construction

An authored Host may explicitly select bounded preparation delegation:

```conduit
preparation = {authorizer: "seed-owner", maximum_assignment_bytes: 8192, maximum_runtime_state_bytes: 8192}
```

This optional singleton selects construction intent. Absence selects no
permission; membership or an owner key alone does not select it. The Seed must
resolve `seed-owner` to its independently retained Body/issuer context. Private
keys do not belong in this source. Unknown authorizers, missing or duplicate
fields/declarations, and nonpositive or excessive byte ceilings refuse. Assignment
bytes must fit the Host's buffered-byte bound; runtime-state bytes must fit its
static-memory bound. These ceilings never enlarge a resource grant.

Canonical Source, configuration identity and IMAGE correspondence retain the
selection. Configurations without it retain their existing representation and
identity. The ordinary Host-configuration Face exposes selected intent; Check and
Rename preserve it, and target replacement refuses before silently discarding
it. `rust_preparation_delegation` emits a typed optional constant from the checked
configuration, without a runtime configuration parser.

This construction surface does not itself approve an assignment, authenticate a
request, reserve memory, issue a native capability or start a Play. The Root must
still implement those boundaries, including exact current Body/Host/Boot scope,
finite local admission, cancellation and retirement. This is a prerequisite of
[#6770](https://github.com/dancxjo/conduit/issues/6770), not its physical acceptance.

## Selecting external execution in Host construction

External execution is a separate optional construction selection:

```conduit
execution = {authorizer: "seed-owner", maximum_active_worksets: 1}
```

It requires selected bounded preparation. Its positive active-workset ceiling
must fit the Host's active-instance limit; it does not enlarge that limit or
any memory, work or native capability grant. Canonical Source, configuration
identity, IMAGE correspondence and generated typed policy retain this choice.
Absence delegates no execution, including when preparation is selected.
The configuration Face exposes the choice and preserves it on ordinary edits.
This declaration alone does not authorize a received assignment or start a
kernel: Root must still bind the pinned issuer to the exact current approved
Body workset, admit its full resources and obtain actual local possessions.

## Selecting bounded continuation permission

A Host can separately opt into owner-signed permission to reconstruct a prepared
workset after reboot. It is an optional field of `execution`:

```conduit
execution = {authorizer: "seed-owner", maximum_active_worksets: 1, continuation: {assignment_bytes: 8192, runtime_bytes: 8192, durable_bytes: 32768, operation_slots: 4, active_worksets: 1}}
```

All five ceilings must be positive. Assignment bytes must fit selected
preparation, runtime bytes the declared heap arena, operation slots the Host
limit, and active worksets the external-execution limit. The durable-byte ceiling
does not declare a storage Base, supply a residence or reserve media. Physical
storage and safe provider work still require independent finite admission.
Absence keeps ordinary external execution scoped to its current Boot.

This choice is retained in canonical Source, configuration identity and generated
typed policy. It supplies no membership receipt or Start. A candidate continuation
intent must match independently installed Body/Part/Host, Source/workset,
IMAGE/configuration, issuer and locally accepted generations. Signature validation
alone does not prove those durable installation facts. Root must reconstruct fresh
canonical identities, inspect actual current providers, admit the complete workset
and issue fresh current-Boot capabilities before execution. An old command or
Started token cannot be replayed. Disconnected operation cannot discover a remote
revocation it has not received. See [#6841](https://github.com/dancxjo/conduit/issues/6841)
for the remaining physical restart acceptance; configuration and intent validation
alone do not provide automatic restart.

## One kernel, explicit effects

[plan lowering](plan-kernel-lowering.md) turns rich identities into finite
numeric tables before play starts. Hosted storage may allocate during
preparation; the admitted execution path cannot grow its queues or hide work
in an unbounded callback stream. Unsupported profiles refuse before execution.

The [execution kernel](../../architecture/kernel) owns scheduling, typed port
traffic, atomic fan-out, pressure, operation correlation, closure, cancellation,
and terminal evidence. Platform adapters perform admitted Host Calls and
return their exact completions. They do not become a second scheduler.

A protected effect also needs current authority at its trusted provider.
[Cooperative authority checks and hostile-code confinement](implementation-confinement.md)
are different guarantees. A plan or serialized grant identity is not by itself
an isolation mechanism.

## bodies, plots, and connections

A [body](body-lifecycle-waists.md) is a durable logical computer with a bounded
workset of zero, one, or many plots. A part is an admitted membership relationship;
a host and boot describe current machinery. Membership, reachability, capability,
and authority do not imply one another.

One body-wide plan covers the workset; one active play realizes it during a wake.
Workload replacement follows the bounded lifecycle transaction, retaining the
body and exact evidence. body continuity is not represented by recursively
pretending that a set of hosts is another host. Reusable composition belongs to
plots and their fores/backs.

A cord is a typed semantic connection. A remote cord uses an exact planned
[line](route-candidates.md), with a separate [session and attachment](session-route-attachment.md).
New availability observations cannot add an unplanned route or invent retry,
reconnect, membership, or authority. [Replacement planning](topology-planning-play-control-loop.md)
creates fresh immutable realization truth.

## Implementation map

| Responsibility | Source |
| --- | --- |
| Shared architecture vocabulary | [`architecture/core`](../../architecture/core) |
| Assigned-plan schema and validation | [`architecture/assigned-plan`](../../architecture/assigned-plan) |
| Source, checking and expansion | [`architecture/plot`](../../architecture/plot) |
| Planning and admission | [`architecture/planner`](../../architecture/planner) |
| Numeric lowering | [`architecture/plan-lowering`](../../architecture/plan-lowering) |
| Shared execution kernel | [`architecture/kernel`](../../architecture/kernel) |
| body membership and lifecycle | [`architecture/body`](../../architecture/body) |
| Read-only runtime projection | [`architecture/observatory`](../../architecture/observatory) |
| Portable hosted implementation | [`targets/std`](../../targets/std) |
| Browser host and runtime | [`targets/browser`](../../targets/browser) |
| Native ConduitOS machinery | [`targets/conduitos`](../../targets/conduitos) |
| RP2040 make and firmware | [`targets/rp2040`](../../targets/rp2040) |

[Make](../host-make.md) selects machinery through
PROFILE → BUILD → IMAGE. Building an image does not create a live host, a boot,
a body membership, or an offer. Runtime observations establish those facts.

The early [portable-host proposal](../history/architecture/portable-hosts.md) and
[CHS-0 specification](../history/architecture/host-specification.md) are retained as historical rationale.
Their composite-host model and milestone requirements are not current API or
contributor requirements.
