# The Conduit canon

**Status:** durable project direction and architectural intent  
**Audience:** maintainers, contributors, coding agents, reviewers, and future users  
**Current executable truth:** [STATUS.md](../STATUS.md)  
**Current work:** [roadmap](roadmap.md)
**Architecture reference:** [topic index](architecture/README.md)

This document exists so that good ideas do not have to become immediate code merely to avoid being forgotten. It records the project Conduit is trying to become, the distinctions it must preserve, the concepts already earned by executable proof, and the larger ideas that remain valuable without yet being current obligations.

The archive, the August reboot, and the current implementation are parts of one history. Code may be replaced while an idea survives; an idea may be retained while its first implementation is retired.

## The center

> **Forms describe meaning. Hosts offer implementations. Plans make realization exact.**

Conduit is a portable execution substrate for finite, typed flows of work.

An author should be able to describe what must happen without deciding which operating system, browser, microcontroller, process, transport, device, or service will realize it. Hosts report what they can currently do. A planner combines the authored meaning with exact current offers, resources, authority, and links. The resulting plan is immutable and complete enough to execute without ambient guesses.

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
PORT   typed directional point through which Info enters or leaves
CORD   typed semantic connection between compatible ports on gears
INFO   shaped, typed data carried through cords
RESOURCE bounded addressable content with explicit lifecycle and sharing obligations
SIGNAL one particular Info semantic or mechanism where explicitly named
FACE   stable visible semantic contract of a kind or form, including ports
BACK   form that implements a face in Conduit terms

IMPL   platform-specific realization of a kind
HOST   running software environment that makes truthful finite offers
PLAN   exact immutable realization of an admitted workload
PLAY   one active execution of a plan
```

A Kind is not a Gear, and neither is an implementation. A Port is not a renderer jack, queue slot, Line endpoint, or Base handle. Info is specifically shaped/typed data and is not automatically Signal. A Face is not its Back or an exact realization. An installed implementation is not necessarily initialized. An initialized implementation is not necessarily advertised. An advertised capability offer is not selected. A selected offer is not reserved. A reservation is not an active Play.

### Resource, State, and Line

A Resource is bounded addressable content whose residence, lifetime, sharing,
access, generation, or durability matters beyond an ordinary inline Info value.
`value/resource-ref@1` remains portable Info referring to exact semantic content
and version; possession grants no authority. Structured records remain Info,
not collections of Gears masquerading as objects.

A Cord carries Info. A Line realizes that Cord's traffic. Shared memory used by
a Line is transport machinery; shared memory backing an explicitly admitted
Resource is residence. Neither introduces a portable pointer, fd, mapping,
device handle, or distributed shared memory. Resource residence belongs to
exact Host/Boot/Base realization and Plan truth.

Published Info names stable Resource generations. Candidate publication and
read leases are finite, explicit contracts; a reader cannot silently observe
mutation beneath immutable content. Unsupported coherence refuses. Another
sealed generation or residence requires fresh exact Plan truth.

State retains evolving Info across an explicit time boundary and may retain a
ResourceRef. Persistence is a Resource operation and lifecycle obligation.
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
Sign item
presentation
```

A spelling-only edit may change source identity without changing checked meaning. A hidden nested implementation may change expanded identity without changing the visible checked contract. A new placement or boot changes the plan. A replay creates a new play. A UI row is not a Sign identity.

### Availability, authority, and relationship

```text
reachable       a Line can currently address an endpoint
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
credential, or other effect authority. Every concrete Play remains finitely
admitted, including its memory, queues, operations, Resources, and mandatory
work. Resource containment is distinct from proving that every future step
will fit or that a computation will terminate.

A grant identity describes authority; a serializable identity does not itself
constitute unforgeable possession. The trusted enforcement boundary must
validate exact current authority independently of a planner's proposed Plan.
Replacement Boot truth requires fresh admission. Signs describe decisions and
effects; they neither grant permission nor prevent an unauthorized effect.

This is a durable requirement, not a claim of existing hostile-code isolation.
A cooperative std process, an isolated implementation with restricted imports,
a native ConduitOS boundary, and a remote authenticated peer have different
attacker assumptions and proof obligations. State the actual mechanism and
trust class; never collapse them into a generic security flag. The
[confinement contract](architecture/implementation-confinement.md) records
these boundaries and current evidence under #2685.

The normal distributable unit of effect authority is a narrow **Base**, not a
privileged universal Host. A thin Host supervisor owns Host/Boot identity,
current offer truth, shared planning participation, finite Play accounting,
capability issuance and revocation, Sign correlation, and Base lifecycle. It
aggregates only the offers of Bases that are installed, configured, current,
and ready. It does not inherit a Base's filesystem, network, process, device,
service, or actuator authority merely because it supervises that Base.

A Base is the last trusted Conduit seam for one bounded family of external
reality. Its provider identity and generation remain distinct from Host, Boot,
resource, Plan, and Play identity. Replacement invalidates stale provider
truth. Registration and discovery communicate availability, never authority;
the shared planner consumes independently valid authority and may only narrow
it. Confinement class is descriptive mechanism truth—such as cooperative,
process-isolated, WASM-confined, OS-capability-mediated, ConduitOS-kernel-
enforced, or hardware-gated—not an ordered score or `secure` boolean.

Integration is not assimilation. An external resource keeps its external
identity, semantics, lifecycle, and native security sovereignty when a Base
maps it to or manifests a Conduit meaning. External identity, semantic Kind,
directional mapping, adapter/Base, authority, and outward manifestation are
never aliases. Import and export are independently configured and authorized;
discovery supplies observation only and cannot fabricate a Host, Part,
capability, membership, trust, or authority.

An outward manifestation that may be observed again carries exact adapter,
mapping, and manifestation origin. Default discovery fences that reflection;
intentional re-import requires a fresh bounded directional mapping and
authority decision. Mapping contracts retain material external type,
lifecycle, ordering, and delivery limits rather than silently promising
stronger Conduit semantics. This membrane is generic to every ecosystem and
does not make ROS, an operating system, a browser, or a service namespace part
of Conduit ontology.

### Execution and presentation

The DOM, stdout, LEDs, dashboards, and workbench canvases are manifestations or projections. They do not own semantic truth, lifecycle truth, plan identity, authority, or Sign.

A presentation may summarize or arrange runtime facts. It may not manufacture them.

Web manifestations use the native HTML control whose semantics match the
operation: checkboxes for independent choices, radio buttons for one choice
among alternatives, selects for bounded lists, buttons for actions, anchors
for navigation, and fieldsets with legends for grouped choices. Presentation
may alter their appearance, but does not recreate native interaction semantics
when the platform control already expresses the operation.

An already-resolved bounded graphics scene may cross one terminal presentation
Face to request manifestation. That Face names no framebuffer, DOM, window,
pixel format, or toolkit object: the selected implementation, admitted host
operation, finite presentation resource, and exact display Base remain Plan and
Host truth. Transform Kinds do not acquire hidden manifestation side effects.

### Fabrication and runtime

A Host fabrication package is a Rust project boundary that knows how to manufacture machinery for a finite coherent family of exact targets. An anchor package owns each target's descriptor, toolchain and build adapter, finite maxima, artifact kinds, and target-appropriate post-build mechanics. Extension packages may add exact Base implementation offers without editing the anchor or generic Conduit fabrication.

```text
fabrication packages present in a project
    -> exact target and implementation offers

Host construction
    -> target + selected Base implementations + finite bounds

PROFILE -> BUILD -> IMAGE
    -> exact machinery plus package, implementation, and tooling provenance

LAUNCH / LOAD / FLASH / BOOT
    -> target-appropriate later actions

HOST / BOOT
    -> runtime truth only after machinery actually comes alive
```

The common contract is not an artifact format, CPU architecture, firmware ontology, or deployment verb. Native bundles, browser bundles, UF2 firmware, ConduitOS disk images, ESP images, and Raspberry Pi SD images remain honestly different. A package may own several exact targets where that is the coherent maintenance boundary; it may not erase their distinct board, architecture, machine, toolchain, artifact, or proof identities.

Package inspection is lightweight. Heavy toolchains and builders run only for BUILD. The package environment is composed explicitly through ordinary Rust dependencies rather than a central closed target list, runtime plugin loader, or package marketplace. Competing implementation offers are explicit; duplicate exact implementation identity refuses rather than resolving by load order.

PROFILE, BUILD, and IMAGE describe and manufacture inert machinery. They do not create HostId, BootId, reachability, membership, authority, live offers, reservations, Plans, or Plays.

## Execution invariants

### Exact typed ports

Every executable input and output has a port identity, direction, and value kind. Values enter through named inputs and leave through named outputs.

Fan-out is an explicit planned property. One emission is admitted atomically to the required branches or waits under pressure. The kernel never interprets an unqualified `emit` as broadcast to whatever happens to be connected.

### Bounded before Play start

Before a play starts, the host knows and admits the finite shape needed for execution:

- gear instances;
- values and bytes;
- cords and routes;
- queue items and buffered bytes;
- timers and host-operation concurrency;
- resource reservations;
- mandatory Sign storage;
- cancellation and terminal bookkeeping.

Hosted profiles may use heap-backed storage before Play start. Constrained profiles may use fixed arenas. Neither may conceal unbounded growth, discovery, retry, string lookup, graph scanning, or queue creation in an admitted hot path.

### Generic host operations

Operations request exact admitted host work such as waiting, presenting a value, reading a resource, writing a resource, or later invoking a device action.

The kernel owns execution order and correlation. The platform adapter owns only the requested platform effect and completion. It does not become another scheduler.

### Honest pressure and failure

Pressure is not an implementation inconvenience to hide with buffering. It is runtime truth.

Values remain accounted for through offered, accepted, delivered, failed, cancelled, or terminal disposition. Disconnect, malformed input, stale boot, authority denial, resource exhaustion, Sign exhaustion, and unsupported behavior remain distinguishable.

Automatic retry is a semantic promise and therefore must be planned. A base may not invent it.

### One kernel

Portable std, browser, Pico, future Android, and ConduitOS profiles use the same execution protocol and scheduler semantics.

Temporary compatibility façades may help migrate old tests or composite fixtures. They must be named as compatibility surfaces, excluded from production paths, and prevented from becoming a permanent second engine.

## General-purpose finite computation

Conduit targets general-purpose computation under explicit finite bounds.
Every checked executable Form has exact finite semantic/resource capacities
after specialization. Reusable algorithms may parameterize those capacities;
a different semantic bound may produce a different checked identity. Large
finite state spaces remain finite but may be impractical to enumerate.

General typed State, branching/selection, explicit recurrence and bounded
structured memory provide computational generality through ordinary typed
composition. Continuous externally driven lifetime requires no special mode
and does not imply unbounded retained State or instantaneous resources.
Strict Turing completeness and semantically infinite memory are not current
requirements. They require a new concrete architectural justification before
introduction, rather than an opt-out flag added for convenience. The
[finite computation contract](architecture/decidable-default-universal-extension.md)
records the revised #2682 direction and its analysis, lifecycle, timing,
continuity and confinement obligations.

## Form and composition direction

A form is semantic source, not platform installation configuration.

A form may contain:

- configured Gears and their Kinds;
- typed cords;
- semantic configuration;
- explicit finite work bounds;
- nested forms;
- named input and output faces;
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

All forms are conceptually composite. A form with one opaque implementation is simply the smallest composition. A nested form becomes substitutable through its checked faces while its hidden expansion remains bound into expanded and plan identity.

A BODY may later appear through a FACE inside another form without becoming a copy of that body.

## Body identity and lifecycle

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
BODY  durable logical computer with a bounded workset of Forms
SOUL  durable continuity and recoverable identity of a body
```

A Body is not a Host, transport, address, coordinator process, or UI document. A Part is not a Host or Boot: it records an explicitly admitted durable relationship. Current authenticated Host/Boot presence may attach to that relationship and later disappear without deleting membership or retaining a fake current Boot. Current offers remain Host-advertisement truth rather than durable Part properties. Admission and revocation carry exact bounded event and Sign provenance; membership alone grants no authority, placement, Line, or execution.

A Body is one logical computer. It may contain one machine or many, and it may
run many Forms. One Body scheduler plans all of that work together; one Play is
the Body's current running realization. The same Body/Plan/Play model covers a
single Host, multiple cores, and multiple Hosts; distribution does not create a
second scheduler or execution ontology.

A Body is a **continuant**. An explicit attributable human/operator action
**BIRTHs** it with a bounded initial workset of zero, one, or many exact checked
Forms and records distinct birth event/Sign evidence. No initial Form is
privileged after birth. In Conduit vocabulary a program is a Form; there is no
separate Program identity. The newborn Body is LULLED; BIRTH creates no implicit
Wake, Plan, or Play. Thereafter changes in Parts, Hosts and Boots, Lines, the
bounded current Form workset, Wake/Lull episodes, Plans, Plays, and
manifestations are events in the history of the same Body rather than
replacement Body identities.

The Form workset may contain zero, one, or many exact checked Forms. Adding or
removing a Form advances bounded workload truth without replacing the Body.
During one Wake, one Body-wide immutable Plan covers the complete current
workset, globally admits its resources, and may have at most one active Play.
Forms inside that Play may progress concurrently under the one kernel. A
workset change retires the current Plan and Play and requires a replacement
Body-wide Plan before execution resumes; it never starts a second scheduler.
Legacy seed-era Body evidence remains explicitly versioned historical evidence;
it does not restore a privileged identity in the current model.

Absence is not death. Offline Parts, unreachable Hosts, lost Boots or Lines,
Lull, stale or missing Plans, terminated Plays, and even loss of all current
realization do not by themselves erase Body continuity. Routine cleanup,
shutdown, garbage collection, or disappearance of current offers must retain
the durable Body and membership evidence. Any future irreversible destruction
protocol requires its own explicit authority and semantics; it is not an
ordinary `delete Body` operation.

The minimum continuity law is one surviving Part: a Body remains the same Body
while at least one admitted Part retains sufficient bounded durable continuity
truth. That truth binds the exact Body, surviving membership relationship and
generation, workload revision where lifecycle law requires it, and durable
authority or revocation provenance needed to reject a stale or copied claim.
Reboot and replacement rotate current Boot, resource, Line, offer, authority,
Plan, and Play truth without replacing the Body. After the final continuity-
bearing Part and its evidence are destroyed, Conduit makes no promise of
same-identity resurrection. SOUL, where used, names only this material and
protocol rather than supernatural recovery after total extinction.

Workload revision is atomic with realization truth. One serialized lifecycle
checks the proposed complete workset, plans and admits it, prepares replacement,
quiesces affected old work, and commits a new immutable revision and Plan. A
refused attempt preserves the coherent prior state or an explicit LULLED state;
it never publishes a hybrid or starts another scheduler. Planned failure
disposition may terminate an exact scope, select a checked degraded path, wait
under finite admission, request this same replacement lifecycle, or lull. It
never implies a hidden retry or a parallel supervisor runtime.

Signs may carry bounded exact causal relationships. These relationships record
what caused, requested, admitted, realized, observed, superseded, corrected, or
terminated exact evidence across exact sessions. Temporal adjacency is not
causality, missing evidence remains unknown, and presentation never owns the
causal history.

A Form contains configured Gears and may require Roles. A Cast binds Roles to exact capabilities. A Body-wide Plan binds every Form's Gears to exact implementation, Part, Host, Boot, Base, authority, resource, route, and bound facts. A Play starts that complete Plan. A later Soul policy may prove continuity across restarts without pretending a restarted Boot is the same execution session or changing what Part membership means.

ConduitOS is a native Host substrate for this same admitted Plan and kernel. It
does not supply an alternate scheduler or kernel semantics. Current ConduitOS
proof uses cooperative execution through the one kernel; it does not yet prove
SMP, preemption, or physical parallel execution.

Membership, reachability, authority, capability, placement, and link state remain separate.

Body membership, continuity, administration, and workload-transition contracts
now have dedicated implementations in `architecture/body`. Product integration
and recovery proof remain distinct from those contracts. The layer consumes
Host reports, Lines, planning, and the one kernel; it does not invent a parallel
distributed runtime. See [Body lifecycle contracts](architecture/body-lifecycle-waists.md)
and the [current roadmap](roadmap.md).

## Proof classes

The repository uses precise proof names:

1. contract or compile proof;
2. deterministic simulation;
3. executable hosted implementation;
4. actual platform adapter or runtime;
5. live transport;
6. actual firmware execution;
7. physical or hardware-in-the-loop Sign.

A Thumb build is not firmware execution. A WASM build is not browser execution. A browser test is not a live network link. A loopback socket is not installation security. An LED blink is not a machine-readable receipt.

These classes are distinct, not a ladder where one automatically substitutes
for another. [The machine-readable proof catalog](architecture/proof-classes.md)
defines the exact vocabulary; `STATUS.md` records the established classes and
remaining gaps for each surface.

## Direction of travel

The dependency direction remains meaning and typed contracts → exact planning
and admission → the execution kernel → Host effects → product presentation.
Body lifecycle coordinates the current workset through those same boundaries.
Domain applications consume them rather than introducing another runtime.

The early S1–S5 salvage sequence is historical. Conduit now has a semantic
catalog, Body lifecycle, browser and native product surfaces, device work, and
ConduitOS emulator execution. Those surfaces have different levels of proof;
their existence is not a claim that every end-to-end journey is complete.
[STATUS.md](../STATUS.md) describes those limits, and the [roadmap](roadmap.md)
links the work currently being pursued.

## The idea vault

Ideas are classified so that deferral does not feel like erasure and preservation does not become accidental scope.

### Living core

These ideas are current, load-bearing direction and have executable implementations and bounded proof surfaces in the repository:

- semantic forms, host capability offers, and exact plans;
- source, checked, expanded, plan, play, Sign, and presentation identity separation;
- typed named ports and explicit fan-out;
- bounded port-aware `conduit-kernel` execution;
- generic host operations;
- exact resource, authority, and observed-link planning contracts;
- lossless source retention and located diagnostics;
- inline nested forms and named composite faces;
- hosted and browser execution through the kernel, with separately proved Lines;
- the portable semantic catalog and Host-owned realization offers;
- Body membership, workload and continuity contracts;
- read-only Observatory and portable Presentation/Manifestation contracts;
- CLI, Tour, Crèche, Patchbay, and ConduitOS product surfaces;
- ConduitOS freestanding emulator execution and retained visual evidence;
- honest proof-class boundaries.

### Dormant, not discarded

These are valuable directions waiting on named prerequisites. An implemented
slice belongs in the living project even when its larger ambition remains open:

- broader SOUL recovery and durable policy beyond bounded continuity evidence;
- Zenoh as a possible later Line Base;
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
- broader durable trust, delegation, and recovery beyond current Body admission protocols;
- the exact package, artifact, and implementation installation workflow;
- how much automatic placement and negotiation belongs above explicit planning;
- how bodies gather on neutral ground without confusing discovery with trust;
- which timing profiles can be admitted across which local regions;
- the final operator experience connecting task faces, Observatory, and Workbench;
- how a body exposes a face into another body while preserving continuity and authority;
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

A user should not need to understand the machinery before receiving value. After the task works, Conduit should make every hidden choice inspectable: source, checked meaning, expansion, plan, placement, resources, authority, play, Sign, and presentation.

The current product loop is:

```text
Enter -> See -> Make -> Rehearse -> Wake -> Observe -> Explain
```

This is one semantic loop across radically different Hosts and Presenters, not
a request for identical pixels or mechanisms. Presentation states what exists,
which ordinary semantic actions are available, why an action is unavailable,
and which exact truth waits behind inspection. A Presenter binds local gestures
such as keys, pointer activation, numbered serial choices, or touch to those
actions; the gesture does not become the meaning or a second mutation path.

The ordinary surface prioritizes the meaningful object, current state, current
action, effect, and refusal. Exact source, Form, Body, Plan, placement, Host,
implementation, Base, Play, and Sign truth remains reachable through explicit
explanation rather than occupying the lobby. Geometry, focus, clipping,
scrolling, and responsive layout remain Presenter-local.

Temporal context follows the same boundary. A Presentation may state how an
exact event, observation, or ingestion instant relates to an exact reference
instant for that Presentation turn, but relative age is derived Presentation
truth rather than event, Sign, or evidence identity. Portable temporal facts
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

Temporal facts name an existing Presentation subject. When they claim Sign
provenance, that Sign is already present in the Presentation basis. A new
reference instant may change Presentation content identity, revision, and the
derived temporal relation without changing the referenced subject, event,
observation, exact instant, or Sign identity. Relative wording, locale,
periodic refresh, clock acquisition, and domain-specific freshness thresholds
remain Presenter or separately reviewed policy concerns; relative strings are
never stored as canonical evidence.

For product work, exact proof is necessary but not sufficient. A completed
slice also demonstrates that a person can enter through a supported product or
repository-development entrance, recognize the intended object, perform the
intended ordinary action, see its correlated effect or refusal, and descend to
the exact proof when curious.

Product demonstrations are ordinary checked Forms travelling through
the real checker, planner, kernel, Presentation, and Presenter. A bespoke demo
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
