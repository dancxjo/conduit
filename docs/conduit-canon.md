# The Conduit canon

**Status:** durable project direction and architectural intent  
**Audience:** maintainers, contributors, coding agents, reviewers, and future users  
**Current executable truth:** [STATUS.md](../STATUS.md)  
**Current work:** [roadmap](roadmap.md)
**Architecture reference:** [topic index](architecture/README.md)

This document exists so that good ideas do not have to become immediate code merely to avoid being forgotten. It records the project Conduit is trying to become, the distinctions it must preserve, the concepts already earned by executable proof, and the larger ideas that remain valuable without yet being current obligations.

The archive, the August reboot, and the current implementation are parts of one history. Code may be replaced while an idea survives; an idea may be retained while its first implementation is retired.

## Writing the vocabulary

Ontology terms are common nouns unless they are independently proper names.
Write “a body has a face,” “a host offers an implementation,” and “a form is
realized by a plan and play.” Preserve capitalization for actual names such as
Conduit, ConduitOS, Patchbay, Tour, and Crèche, and for literal code identifiers
such as `BodyId`, `CheckedFront`, and `Form`.

## The center

> **forms describe meaning. hosts offer implementations. plans make realization exact.**

The canonical execution vocabulary keeps one noun at each altitude:

```text
a gear invokes a kind through its front
a host offers a back for that kind
a plan chooses exact backs
a play advances the plan in bounded steps
a step may cross into host machinery with a host call
```

A `Kind` is the exact semantic contract: `KindId`, `KindIdentity`, callable
Front, finite configuration contract, limits, and machine-readable semantic
laws. An identity is not merely a human version label. `KindProjection` is the
strictly smaller checker view used while checking authored Forms; it cannot be
offered by a Host and is not a second Kind identity.

A `Back` is realization truth, not another semantic contract. Host backs carry
implementation, artifact, resource, authority, and host-boundary requirements.
Form backs carry the exact source and checked form selected during expansion.
They share the law “same Kind and same Front,” but remain distinct
representations because their provenance and admission facts differ.

“Capability” is reserved for possession/authority where that security meaning
is real, such as `BaseCapability`. A Host advertisement may still serialize a
capability offer for compatibility, but the offered realization is a Back.

Conduit is a portable execution substrate for finite, typed flows of work.

An author should be able to describe what must happen without deciding which operating system, browser, microcontroller, process, transport, device, or service will realize it. hosts report what they can currently do. A planner combines the authored meaning with exact current offers, resources, authority, and links. The resulting plan is immutable and complete enough to execute without ambient guesses.

The same form may therefore run:

- inside one portable Rust process;
- in an actual browser runtime;
- on a constrained microcontroller;
- across several connected hosts;
- inside a robot composed of cooperating parts;
- inside a native ConduitOS execution image.

These are realization directions, not interchangeable acceptance claims. Current
ConduitOS execution includes freestanding emulator proof; physical deployment
and each target's supported operations have separate evidence in `STATUS.md`.

Portability does not mean pretending those environments are identical. Their clocks, memory, implementations, links, physical effects, limits, and failures remain explicit in capabilities and plans.

## What Conduit is not

Conduit is not merely:

- a visual node editor;
- a workflow service;
- Terraform with arrows;
- a message broker;
- an actor framework;
- a browser application;
- an RTOS;
- a robotics stack;
- or a package catalog.

It may support surfaces resembling all of those. They remain users or profiles of the same substrate. None may introduce a second graph, scheduler, authority model, or source of runtime truth.

## Durable separations

The project succeeds by refusing to collapse concepts that are convenient to conflate.

### Meaning and realization

```text
KIND   reusable semantic behavior such as text/upper
FORM   authored composition of semantic work; the program Conduit runs
GEAR   one configured occurrence of a kind in a form
PORT   typed directional point through which info enters or leaves
CORD   typed semantic connection between compatible ports on gears
INFO   shaped, typed data carried through cords
RESOURCE bounded addressable content with explicit lifecycle and sharing obligations
SIGNAL one particular info semantic or mechanism where explicitly named
FRONT   stable callable shape of a kind or form, including startup parameters and ports
BACK    one Host- or Form-backed realization of a kind
FACE   the body's semantic presentation surface, distinct from a callable front

IMPL   platform-specific realization of a kind
HOST   running software environment that makes truthful finite offers
PLAN   exact immutable realization of an admitted workload
PLAY   one active execution of a plan
```

A kind is not a gear, and neither is an implementation. A port is not a renderer jack, queue slot, line endpoint, or base handle. info is specifically shaped/typed data and is not automatically Signal. A front is not its back or an exact realization. An installed implementation is not necessarily initialized. An initialized implementation is not necessarily advertised. An advertised capability offer is not selected. A selected offer is not reserved. A reservation is not an active play.

### resource, state, and line

A resource is bounded addressable content whose residence, lifetime, sharing,
access, generation, or durability matters beyond an ordinary inline info value.
`value/resource-ref@1` remains portable info referring to exact semantic content
and version; possession grants no authority. Structured records remain info,
not collections of gears masquerading as objects.

A cord carries info. A line realizes that cord's traffic. Shared memory used by
a line is transport machinery; shared memory backing an explicitly admitted
resource is residence. Neither introduces a portable pointer, fd, mapping,
device handle, or distributed shared memory. resource residence belongs to
exact host/boot/base realization and plan truth.

Published info names stable resource generations. Candidate publication and
read leases are finite, explicit contracts; a reader cannot silently observe
mutation beneath immutable content. Unsupported coherence refuses. Another
sealed generation or residence requires fresh exact plan truth.

state retains evolving info across an explicit time boundary and may retain a
ResourceRef. Persistence is a resource operation and lifecycle obligation.
Recording retains historical evidence under its own contract. Retain, persist,
and record are distinct; there is no universal `save` primitive.

### Identity stages

These identities remain distinct even when a small example makes them appear interchangeable:

```text
source document
checked form
expanded form
plan
plan fragment
active play
sign item
presentation
```

A spelling-only edit may change source identity without changing checked meaning. A hidden nested implementation may change expanded identity without changing the visible checked contract. A new placement or boot changes the plan. A replay creates a new play. A UI row is not a sign identity.

### Availability, authority, and relationship

```text
reachable       a line can currently address an endpoint
observed link   one exact boot-scoped path is currently available
member          a participant belongs to a larger durable relationship
trusted         an authority decision permits some action
capable         a current offer can realize a semantic contract
selected        an exact plan chose that offer
```

None implies the next.

A discovered device is not automatically a host. A host on the network is not automatically a member. A member is not automatically authorized. A resource being free is not permission to affect it.

### Least authority and finite embodiment

An implementation must not possess materially more effect authority than the
exact admitted realization it executes. General-purpose computation and
continuous lifetime grant no filesystem, network, device, subprocess,
credential, or other effect authority. Every concrete play remains finitely
admitted, including its memory, queues, operations, resources, and mandatory
work. resource containment is distinct from proving that every future step
will fit or that a computation will terminate.

A grant identity describes authority; a serializable identity does not itself
constitute unforgeable possession. The trusted enforcement boundary must
validate exact current authority independently of a planner's proposed plan.
Replacement boot truth requires fresh admission. signs describe decisions and
effects; they neither grant permission nor prevent an unauthorized effect.

This is a durable requirement, not a claim of existing hostile-code isolation.
A cooperative std process, an isolated implementation with restricted imports,
a native ConduitOS boundary, and a remote authenticated peer have different
attacker assumptions and proof obligations. state the actual mechanism and
trust class; never collapse them into a generic security flag. The
[confinement contract](architecture/implementation-confinement.md) records
these boundaries and current evidence under #2685.

The normal distributable unit of effect authority is a narrow **base**, not a
privileged universal host. A thin host supervisor owns host/boot identity,
current offer truth, shared planning participation, finite play accounting,
capability issuance and revocation, sign correlation, and base lifecycle. It
aggregates only the offers of bases that are installed, configured, current,
and ready. It does not inherit a base's filesystem, network, process, device,
service, or actuator authority merely because it supervises that base.

A base is the last trusted Conduit seam for one bounded family of external
reality. Its provider identity and generation remain distinct from host, boot,
resource, plan, and play identity. Replacement invalidates stale provider
truth. Registration and discovery communicate availability, never authority;
the shared planner consumes independently valid authority and may only narrow
it. Confinement class is descriptive mechanism truth—such as cooperative,
process-isolated, WASM-confined, OS-capability-mediated, ConduitOS-kernel-
enforced, or hardware-gated—not an ordered score or `secure` boolean.

Integration is not assimilation. An external resource keeps its external
identity, semantics, lifecycle, and native security sovereignty when a base
maps it to or manifests a Conduit meaning. External identity, semantic kind,
directional mapping, adapter/base, authority, and outward manifestation are
never aliases. Import and export are independently configured and authorized;
discovery supplies observation only and cannot fabricate a host, part,
capability, membership, trust, or authority.

An outward manifestation that may be observed again carries exact adapter,
mapping, and manifestation origin. Default discovery fences that reflection;
intentional re-import requires a fresh bounded directional mapping and
authority decision. Mapping contracts retain material external type,
lifecycle, ordering, and delivery limits rather than silently promising
stronger Conduit semantics. This membrane is generic to every ecosystem and
does not make ROS, an operating system, a browser, or a service namespace part
of Conduit ontology.

Remote authentication attributes a claim; it grants no membership, trust,
planning eligibility, or effect authority. A remote effect receiver binds the
current peer host/boot/offer generation and exact line/session, fences replay,
then independently validates current body membership and an exact local base
capability at the receiving provider. Authority is non-transitive across
peers: a capability on A→B does not authorize B→C, proxying, delegation, or
failover unless each hop has its own explicit bounded contract and current
authority. Transport confidentiality, peer authentication, membership/line
admission, and effect authorization remain separate inspection facts.

### Execution and presentation

The DOM, stdout, LEDs, dashboards, and workbench canvases are manifestations or projections. They do not own semantic truth, lifecycle truth, plan identity, authority, or sign.

A presentation may summarize or arrange runtime facts. It may not manufacture them.

Web manifestations use the native HTML control whose semantics match the
operation: checkboxes for independent choices, radio buttons for one choice
among alternatives, selects for bounded lists, buttons for actions, anchors
for navigation, and fieldsets with legends for grouped choices. presentation
may alter their appearance, but does not recreate native interaction semantics
when the platform control already expresses the operation.

An already-resolved bounded graphics scene may cross one terminal presentation
front to request manifestation. That front names no framebuffer, DOM, window,
pixel format, or toolkit object: the selected implementation, admitted host
operation, finite presentation resource, and exact display base remain plan and
host truth. Transform kinds do not acquire hidden manifestation side effects.

### Fabrication and runtime

A host fabrication package is a Rust project boundary that knows how to manufacture machinery for a finite coherent family of exact targets. An anchor package owns each target's descriptor, toolchain and build adapter, finite maxima, artifact kinds, and target-appropriate post-build mechanics. Extension packages may add exact base implementation offers without editing the anchor or generic Conduit fabrication.

```text
fabrication packages present in a project
    -> exact target and implementation offers

host construction
    -> target + selected base implementations + finite bounds

PROFILE -> BUILD -> IMAGE
    -> exact machinery plus package, implementation, and tooling provenance

LAUNCH / LOAD / FLASH / BOOT
    -> target-appropriate later actions

HOST / BOOT
    -> runtime truth only after machinery actually comes alive
```

The common contract is not an artifact format, CPU architecture, firmware ontology, or deployment verb. Native bundles, browser bundles, UF2 firmware, ConduitOS disk images, ESP images, and Raspberry Pi SD images remain honestly different. A package may own several exact targets where that is the coherent maintenance boundary; it may not erase their distinct board, architecture, machine, toolchain, artifact, or proof identities.

Package inspection is lightweight. Heavy toolchains and builders run only for BUILD. The package environment is composed explicitly through ordinary Rust dependencies rather than a central closed target list, runtime plugin loader, or package marketplace. Competing implementation offers are explicit; duplicate exact implementation identity refuses rather than resolving by load order.

PROFILE, BUILD, and IMAGE describe and manufacture inert machinery. They do not create HostId, BootId, reachability, membership, authority, live offers, reservations, plans, or plays.

## Execution invariants

### Exact typed ports

Every executable input and output has a port identity, direction, and value kind. Values enter through named inputs and leave through named outputs.

Fan-out is an explicit planned property. One emission is admitted atomically to the required branches or waits under pressure. The kernel never interprets an unqualified `emit` as broadcast to whatever happens to be connected.

Authored and runtime numeric meaning uses four distinct layers. `Count` is a
nonnegative cardinality, index, or finite whole-number count. `Scalar` is a
dimensionless signed fixed-point value. `Quantity` is an exact integer paired
with a reviewed physical or dimensional unit; startup configuration retains
that value and unit through checking, planning, and realization. A domain Info
record supplies the surrounding context—such as frame, source, freshness, or
provenance—and may contain or expose Quantities without collapsing into one.
Targets refuse incompatible or inexact conversion rather than reconstructing a
unit from a parameter name.

`CharacteristicUnit` is narrower realization-selection vocabulary for finite
offer characteristics and preserves the ownership and stability law of those
offers. Count-like domains such as tokens, frames, items, and identifiers may
remain there. It is not a second author-facing physical-unit catalog and must
not grow new physical dimensions that belong to `QuantityUnit`.

### Bounded before play start

Before a play starts, the host knows and admits the finite shape needed for execution:

- gear instances;
- values and bytes;
- cords and routes;
- queue items and buffered bytes;
- timers and host-operation concurrency;
- resource reservations;
- mandatory sign storage;
- cancellation and terminal bookkeeping.

Hosted profiles may use heap-backed storage before play start. Constrained profiles may use fixed arenas. Neither may conceal unbounded growth, discovery, retry, string lookup, graph scanning, or queue creation in an admitted hot path.

### Generic host operations

Operations request exact admitted host work such as waiting, presenting a value, reading a resource, writing a resource, or later invoking a device action.

The kernel owns execution order and correlation. The platform adapter owns only the requested platform effect and completion. It does not become another scheduler.

### Honest pressure and failure

Pressure is not an implementation inconvenience to hide with buffering. It is runtime truth.

Values remain accounted for through offered, accepted, delivered, failed, cancelled, or terminal disposition. Disconnect, malformed input, stale boot, authority denial, resource exhaustion, sign exhaustion, and unsupported behavior remain distinguishable.

Automatic retry is a semantic promise and therefore must be planned. A base may not invent it.

### One kernel

Portable std, browser, Pico, future Android, and ConduitOS profiles use the same execution protocol and scheduler semantics.

Temporary compatibility façades may help migrate old tests or composite fixtures. They must be named as compatibility surfaces, excluded from production paths, and prevented from becoming a permanent second engine.

## General-purpose finite computation

Conduit targets general-purpose computation under explicit finite bounds.
Every checked executable form has exact finite semantic/resource capacities
after specialization. Reusable algorithms may parameterize those capacities;
a different semantic bound may produce a different checked identity. Large
finite state spaces remain finite but may be impractical to enumerate.

General typed state, branching/selection, explicit recurrence and bounded
structured memory provide computational generality through ordinary typed
composition. Continuous externally driven lifetime requires no special mode
and does not imply unbounded retained state or instantaneous resources.
Strict Turing completeness and semantically infinite memory are not current
requirements. They require a new concrete architectural justification before
introduction, rather than an opt-out flag added for convenience. The
[finite computation contract](architecture/decidable-default-universal-extension.md)
records the revised #2682 direction and its analysis, lifecycle, timing,
continuity and confinement obligations.

## form and composition direction

A form is semantic source, not platform installation configuration.

A form may contain:

- configured gears and their kinds;
- typed cords;
- semantic configuration;
- explicit finite work bounds;
- nested forms;
- named input and output fronts;
- semantic requirements that truly belong to the work.

A form does not contain:

- exact hosts or boots;
- implementation IDs;
- addresses or URLs;
- transports;
- device paths or pins;
- DOM selectors;
- stdout;
- credentials;
- resource handles;
- authority grants.

All forms are conceptually composite. A form with one opaque implementation is simply the smallest composition. A nested form becomes substitutable through its checked fronts while its hidden expansion remains bound into expanded and plan identity.

A BODY may later appear through a FRONT inside another form without becoming a copy of that body.

## body identity and lifecycle

The retired realm table is not the intended durable model.

The vocabulary is:

```text
HOST  a running or recoverable software environment
BOOT  one exact current incarnation of a host
PART  one durable membership relationship inside a body
CAPABILITY  truthful finite current realization offer from a host boot
ROLE  semantic requirement declared by a form
CAST  exact binding of roles to capabilities
LINK  communication path between parts
BODY  durable logical computer with a bounded workset of forms
SOUL  durable continuity and recoverable identity of a body
```

A body is not a host, transport, address, coordinator process, or UI document. A part is not a host or boot: it records an explicitly admitted durable relationship. Current authenticated host/boot presence may attach to that relationship and later disappear without deleting membership or retaining a fake current boot. Current offers remain host-advertisement truth rather than durable part properties. Admission and revocation carry exact bounded event and sign provenance; membership alone grants no authority, placement, line, or execution.

A body is one logical computer. It may contain one machine or many, and it may
run many forms. One body scheduler plans all of that work together; one play is
the body's current running realization. The same body/plan/play model covers a
single host, multiple cores, and multiple hosts; distribution does not create a
second scheduler or execution ontology.

A body is a **continuant**. An explicit attributable human/operator action
**BIRTHs** it with a bounded initial workset of zero, one, or many exact checked
forms and records distinct birth event/sign evidence. No initial form is
privileged after birth. In Conduit vocabulary a program is a form; there is no
separate Program identity. The newborn body is LULLED; BIRTH creates no implicit
wake, plan, or play. Thereafter changes in parts, hosts and boots, lines, the
bounded current form workset, wake/lull episodes, plans, plays, and
manifestations are events in the history of the same body rather than
replacement body identities.

The form workset may contain zero, one, or many exact checked forms. Adding or
removing a form advances bounded workload truth without replacing the body.
During one wake, one body-wide immutable plan covers the complete current
workset, globally admits its resources, and may have at most one active play.
forms inside that play may progress concurrently under the one kernel. A
workset change retires the current plan and play and requires a replacement
body-wide plan before execution resumes; it never starts a second scheduler.
Legacy seed-era body evidence remains explicitly versioned historical evidence;
it does not restore a privileged identity in the current model.

Absence is not death. Offline parts, unreachable hosts, lost boots or lines,
lull, stale or missing plans, terminated plays, and even loss of all current
realization do not by themselves erase body continuity. Routine cleanup,
shutdown, garbage collection, or disappearance of current offers must retain
the durable body and membership evidence. Any future irreversible destruction
protocol requires its own explicit authority and semantics; it is not an
ordinary `delete body` operation.

The minimum continuity law is one surviving part: a body remains the same body
while at least one admitted part retains sufficient bounded durable continuity
truth. That truth binds the exact body, surviving membership relationship and
generation, workload revision where lifecycle law requires it, and durable
authority or revocation provenance needed to reject a stale or copied claim.
Reboot and replacement rotate current boot, resource, line, offer, authority,
plan, and play truth without replacing the body. After the final continuity-
bearing part and its evidence are destroyed, Conduit makes no promise of
same-identity resurrection. SOUL, where used, names only this material and
protocol rather than supernatural recovery after total extinction.

Workload revision is atomic with realization truth. One serialized lifecycle
checks the proposed complete workset, plans and admits it, prepares replacement,
quiesces affected old work, and commits a new immutable revision and plan. A
refused attempt preserves the coherent prior state or an explicit LULLED state;
it never publishes a hybrid or starts another scheduler. Planned failure
disposition may terminate an exact scope, select a checked degraded path, wait
under finite admission, request this same replacement lifecycle, or lull. It
never implies a hidden retry or a parallel supervisor runtime.

signs may carry bounded exact causal relationships. These relationships record
what caused, requested, admitted, realized, observed, superseded, corrected, or
terminated exact evidence across exact sessions. Temporal adjacency is not
causality, missing evidence remains unknown, and presentation never owns the
causal history.

A form contains configured gears and may require Roles. A Cast binds Roles to exact capabilities. A body-wide plan binds every form's gears to exact implementation, part, host, boot, base, authority, resource, route, and bound facts. A play starts that complete plan. A later Soul policy may prove continuity across restarts without pretending a restarted boot is the same execution session or changing what part membership means.

ConduitOS is a native host substrate for this same admitted plan and kernel. It
does not supply an alternate scheduler or kernel semantics. Current ConduitOS
proof uses cooperative execution through the one kernel; it does not yet prove
SMP, preemption, or physical parallel execution.

Membership, reachability, authority, capability, placement, and link state remain separate.

body membership, continuity, administration, and workload-transition contracts
now have dedicated implementations in `architecture/body`. Product integration
and recovery proof remain distinct from those contracts. The layer consumes
host reports, lines, planning, and the one kernel; it does not invent a parallel
distributed runtime. See [body lifecycle contracts](architecture/body-lifecycle-waists.md)
and the [current roadmap](roadmap.md).

## Proof classes

The repository uses precise proof names:

1. contract or compile proof;
2. deterministic simulation;
3. executable hosted implementation;
4. actual platform adapter or runtime;
5. live transport;
6. actual firmware execution;
7. physical or hardware-in-the-loop sign.

A Thumb build is not firmware execution. A WASM build is not browser execution. A browser test is not a live network link. A loopback socket is not installation security. An LED blink is not a machine-readable receipt.

These classes are distinct, not a ladder where one automatically substitutes
for another. [The machine-readable proof catalog](architecture/proof-classes.md)
defines the exact vocabulary; `STATUS.md` records the established classes and
remaining gaps for each surface.

## Direction of travel

The dependency direction remains meaning and typed contracts → exact planning
and admission → the execution kernel → host effects → product presentation.
body lifecycle coordinates the current workset through those same boundaries.
Domain applications consume them rather than introducing another runtime.

The early S1–S5 salvage sequence is historical. Conduit now has a semantic
catalog, body lifecycle, browser and native product surfaces, device work, and
ConduitOS emulator execution. Those surfaces have different levels of proof;
their existence is not a claim that every end-to-end journey is complete.
[STATUS.md](../STATUS.md) describes those limits, and the [roadmap](roadmap.md)
links the work currently being pursued.

## The idea vault

Ideas are classified so that deferral does not feel like erasure and preservation does not become accidental scope.

### Living core

These ideas are current, load-bearing direction and have executable implementations and bounded proof surfaces in the repository:

- semantic forms, host capability offers, and exact plans;
- source, checked, expanded, plan, play, sign, and presentation identity separation;
- typed named ports and explicit fan-out;
- bounded port-aware `conduit-kernel` execution;
- generic host operations;
- exact resource, authority, and observed-link planning contracts;
- lossless source retention and located diagnostics;
- inline nested forms and named composite fronts;
- hosted and browser execution through the kernel, with separately proved lines;
- the portable semantic catalog and host-owned realization offers;
- body membership, workload and continuity contracts;
- read-only Observatory and portable presentation/Manifestation contracts;
- CLI, Tour, Crèche, Patchbay, and ConduitOS product surfaces;
- ConduitOS freestanding emulator execution and retained visual evidence;
- honest proof-class boundaries.

### Dormant, not discarded

These are valuable directions waiting on named prerequisites. An implemented
slice belongs in the living project even when its larger ambition remains open:

- broader SOUL recovery and durable policy beyond bounded continuity evidence;
- Zenoh as a possible later line base;
- general package and artifact distribution beyond reviewed fabrication packages;
- domain expansion beyond the current House, Laptop, and Pete slices.

Hostile isolation, physical coverage, and product usability still have concrete
proof gaps; their current scope belongs in the roadmap rather than being
automatically classified as dormant.

Pico firmware, the standard catalog, Observatory, file copy, speech/audio work,
and ConduitOS are no longer dormant ideas. Their remaining proof and integration
gaps belong in the roadmap, not in a list implying no implementation exists.

Dormant ideas should have an owning issue, dependencies, and a future proof. They should not leak placeholder abstractions into the current layer.

### Superseded experiments

These remain useful quarry material but are not to be restored wholesale:

- the broadcast-output reboot runtime;
- the centralized realm membership table;
- enum-only transport selection that manufactured connectivity;
- connection-shaped composite exports;
- the unsafe first copy-file implementation;
- UI-owned runtime truth and task authority;
- giant base and catalog inventories added before one executable vertical slice;
- browser retry matrices and screenshot timing as acceptance;
- direct platform facts in authored source;
- the entire pre-reboot workspace as one indivisible recovery target.

A superseded experiment may contain an excellent algorithm, vocabulary lesson, codec, or proof method. Reuse the smallest reviewed part and record it in `docs/reuse-ledger.md`.

### Unresolved dreams

These remain intentionally open questions:

- future language ergonomics beyond the current canonical `.conduit` syntax;
- broader durable trust, delegation, and recovery beyond current body admission protocols;
- the exact package, artifact, and implementation installation workflow;
- how much automatic placement and negotiation belongs above explicit planning;
- how bodies gather on neutral ground without confusing discovery with trust;
- which timing profiles can be admitted across which local regions;
- the final operator experience connecting task fronts, Observatory, and Workbench;
- how a body exposes a front into another body while preserving continuity and authority;
- how much infrastructure installation Conduit should replace rather than compose with.

An unresolved dream is not a promise and not a rejection. It is a question whose answer must eventually be made executable.

## Recovery rule

Old work returns only when a current vertical slice demands it.

For each recovered concept:

1. identify the archived source and the lesson worth retaining;
2. state what is deliberately excluded;
3. adapt it to current identities, bounds, and kernel contracts;
4. add positive and negative conformance;
5. record the provenance in the reuse ledger;
6. accept the claim only at exact green `main`.

The archive is a quarry, not a branch to merge. The canon is a seed vault, not a mandate to plant everything at once.

## Product direction

Conduit should become useful from the outside inward:

- first prove that meaning survives different hosts;
- then prove that one exact plan crosses real links;
- then provide a small trustworthy vocabulary for building useful forms;
- then let operators see reality through Observatory;
- then let an unfamiliar person complete one useful task;
- only then grow the freeform Workbench and larger domains.

A user should not need to understand the machinery before receiving value. After the task works, Conduit should make every hidden choice inspectable: source, checked meaning, expansion, plan, placement, resources, authority, play, sign, and presentation.

The current product loop is:

```text
Enter -> See -> Make -> Rehearse -> wake -> Observe -> Explain
```

This is one semantic loop across radically different hosts and Presenters, not
a request for identical pixels or mechanisms. presentation states what exists,
which ordinary semantic actions are available, why an action is unavailable,
and which exact truth waits behind inspection. A Presenter binds local gestures
such as keys, pointer activation, numbered serial choices, or touch to those
actions; the gesture does not become the meaning or a second mutation path.

The ordinary surface prioritizes the meaningful object, current state, current
action, effect, and refusal. Exact source, form, body, plan, placement, host,
implementation, base, play, and sign truth remains reachable through explicit
explanation rather than occupying the lobby. Geometry, focus, clipping,
scrolling, and responsive layout remain Presenter-local.

Temporal context follows the same boundary. A presentation may state how an
exact event, observation, or ingestion instant relates to an exact reference
instant for that presentation turn, but relative age is derived presentation
truth rather than event, sign, or evidence identity. Portable temporal facts
therefore retain the exact source instant, its finite clock-basis identity and
resolution, its admitted uncertainty, and its semantic time role. They refer to
one member of a bounded collection of exact, identified reference instants;
there is no ambient or unqualified `now`.

The first portable temporal contract compares only instants in the same clock
basis. Different bases remain machine-readably incomparable unless a separate
reviewed contract admits an exact correlation; reachability or convenient host
clock access does not establish one. Resolution states what a representation
can express, while uncertainty states what is known, and the two remain
distinct. Checked arithmetic yields past, exact/equivalent present, future, or
an explicit indeterminate relation when admitted uncertainty overlaps the
reference. Clock-basis mismatch, absent references, incompatible scales, and
overflow remain typed refusals rather than approximate prose.

Temporal facts name an existing presentation subject. When they claim sign
provenance, that sign is already present in the presentation basis. A new
reference instant may change presentation content identity, revision, and the
derived temporal relation without changing the referenced subject, event,
observation, exact instant, or sign identity. Relative wording, locale,
periodic refresh, clock acquisition, and domain-specific freshness thresholds
remain Presenter or separately reviewed policy concerns; relative strings are
never stored as canonical evidence.

For product work, exact proof is necessary but not sufficient. A completed
slice also demonstrates that a person can enter through a supported product or
repository-development entrance, recognize the intended object, perform the
intended ordinary action, see its correlated effect or refusal, and descend to
the exact proof when curious.

Product demonstrations are ordinary checked forms travelling through
the real checker, planner, kernel, presentation, and Presenter. A bespoke demo
appliance may diagnose a lower boundary, but it does not define the product
experience. The product priority is to make the enactment loop understandable before
expanding the interface. The roadmap records the actual sequencing; this design
principle is not a blanket declaration that all domain work is paused.

## Governance of this canon

This document is durable but not sacred text.

A change to a central invariant or vocabulary should:

- have an explicit architecture issue;
- explain which executable problem requires the change;
- compare the replacement against the current separation of concerns;
- identify migration and proof consequences;
- update `AGENTS.md` when collaboration rules change;
- preserve the previous rationale in history.

Ordinary feature PRs should conform to the canon rather than rewriting it incidentally.

Conduit does not need every good idea alive at once. It needs a trustworthy spine strong enough that the right ideas can return without collapsing the whole creature.
