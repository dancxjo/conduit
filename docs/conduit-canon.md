# The Conduit canon

**Status:** durable project direction and architectural intent  
**Audience:** maintainers, contributors, coding agents, reviewers, and future users  
**Current executable truth:** [STATUS.md](../STATUS.md)  
**Forward sequence:** [issue #1192](https://github.com/dancxjo/conduit/issues/1192)

This document exists so that good ideas do not have to become immediate code merely to avoid being forgotten. It records the project Conduit is trying to become, the distinctions it must preserve, the concepts already earned by executable proof, and the larger ideas that remain valuable without yet being current obligations.

The archive, the August reboot, and the current salvage tree are all parts of one history. None is the entire project. Code may be replaced while an idea survives; an idea may be retained while its first implementation is retired.

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
- or eventually under a static ConduitOS execution image.

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

### Execution and presentation

The DOM, stdout, LEDs, dashboards, and future Workbench canvases are manifestations or projections. They do not own semantic truth, lifecycle truth, plan identity, authority, or Sign.

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

## The future BODY layer

The retired realm table is not the intended durable model.

The forward vocabulary is:

```text
HOST  a running or recoverable software environment
BOOT  one exact current incarnation of a host
PART  one durable membership relationship inside a body
CAPABILITY  truthful finite current realization offer from a host boot
ROLE  semantic requirement declared by a form
CAST  exact binding of roles to capabilities
LINK  communication path between parts
BODY  durable top-level realization of a form
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

A Form contains configured Gears and may require Roles. A Cast binds Roles to exact capabilities. A Body-wide Plan binds every Form's Gears to exact implementation, Part, Host, Boot, Base, authority, resource, route, and bound facts. A Play starts that complete Plan. A later Soul policy may prove continuity across restarts without pretending a restarted Boot is the same execution session or changing what Part membership means.

ConduitOS is a native Host substrate for this same admitted Plan and kernel. It
does not supply an alternate scheduler or kernel semantics. Current ConduitOS
proof is one cooperative execution lane; it does not yet prove SMP,
preemption, or physical parallel execution.

Membership, reachability, authority, capability, placement, and link state remain separate.

This layer waits until Host reports, live Links, the small standard catalog, and Signs are trustworthy. It must consume the kernel rather than invent a parallel distributed world.

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

`STATUS.md` records the highest proof class currently established for each surface.

## Direction of travel

The salvage sequence is intentionally layered:

1. trustworthy port-aware bounded kernel;
2. exact planning over contracts, resources, authority, and observed links;
3. lossless authored forms, nesting, and named faces;
4. actual std, browser, and Pico hosts with bounded live links;
5. a small executable `conduit.std` catalog;
6. BODY, PART, CAPABILITY, ROLE, CAST, LINK, and SOUL;
7. Observatory over real reports, then useful tasks and Workbench;
8. domain proofs such as Tongues and the historical Netherwick project;
9. deadline-bounded regions and ConduitOS only after the bounded execution image is mature.

A later layer may inspire interfaces in an earlier one, but it may not bypass the earlier layer's acceptance gate.

## The idea vault

Ideas are classified so that deferral does not feel like erasure and preservation does not become accidental scope.

### Living core

These ideas are current, load-bearing direction and have executable proof in the salvage tree:

- semantic forms, host capability offers, and exact plans;
- source, checked, expanded, plan, play, Sign, and presentation identity separation;
- typed named ports and explicit fan-out;
- bounded port-aware `conduit-kernel` execution;
- generic host operations;
- exact resource, authority, and observed-link planning contracts;
- lossless source retention and located diagnostics;
- inline nested forms and named composite faces;
- production std and browser-local Signal execution through the kernel;
- honest proof-class boundaries.

### Dormant, not discarded

These are valuable directions waiting on named prerequisites:

- actual Pico W firmware and physical LED receipts;
- live std-browser and std-Pico links;
- Zenoh as a later connection base, not an architectural foundation;
- the small executable standard catalog;
- later SOUL continuity policy beyond current Body lifecycle evidence;
- a real Host Observatory over authoritative reports;
- Copy a file as the first unfamiliar-user task;
- the Workbench as an authoring client of forms and the runtime;
- test receipts and resumable CI;
- the Pico W AP, DHCP, DNS, and HTTP appliance after the smaller Pico proof;
- Tongues as a speech/audio brownfield profile;
- the historical Netherwick robotics experiment as provenance for Pete's
  describe-only, then HIL-safe robotics profiles;
- deadline-bounded regions and static ConduitOS execution images;
- package and artifact distribution once exact installation and authority contracts are ready.

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

- the final public spelling of the form language;
- the durable admission and cryptographic model for bodies;
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

The user should not need to understand the machinery before receiving value. After the task works, Conduit should make every hidden choice inspectable: source, checked meaning, expansion, plan, placement, resources, authority, play, Sign, and presentation.

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
experience. Until the golden enactment loop is understandable, additional
architecture productization and broad domain expansion are paused rather than
replicating an unstable interface.

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
