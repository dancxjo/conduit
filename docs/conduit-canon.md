# The Conduit canon

**Status:** durable project direction and architectural intent  
**Audience:** maintainers, contributors, coding agents, reviewers, and future users  
**Current executable truth:** [STATUS.md](../STATUS.md)  
**Forward sequence:** [issue #361](https://github.com/dancxjo/conduit/issues/361)

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
FORM   authored composition of semantic work
GEAR   one configured occurrence of a kind in a form
PORT   typed directional point through which Info enters or leaves
CORD   typed semantic connection between compatible ports on gears
INFO   shaped, typed data carried through cords
SIGNAL one particular Info semantic or mechanism where explicitly named
FACE   stable visible semantic contract of a kind or form, including ports
BACK   form that implements a face in Conduit terms

IMPL   platform-specific realization of a kind
HOST   running software environment that makes truthful finite offers
PLAN   exact immutable realization of a form
PLAY   one active execution of a plan
```

A Kind is not a Gear, and neither is an implementation. A Port is not a renderer jack, queue slot, Line endpoint, or Base handle. Info is specifically shaped/typed data and is not automatically Signal. A Face is not its Back or an exact realization. An installed implementation is not necessarily initialized. An initialized implementation is not necessarily advertised. An advertised capability offer is not selected. A selected offer is not reserved. A reservation is not an active Play.

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

### Execution and presentation

The DOM, stdout, LEDs, dashboards, and future Workbench canvases are manifestations or projections. They do not own semantic truth, lifecycle truth, plan identity, authority, or Sign.

A presentation may summarize or arrange runtime facts. It may not manufacture them.

An already-resolved bounded graphics scene may cross one terminal presentation
Face to request manifestation. That Face names no framebuffer, DOM, window,
pixel format, or toolkit object: the selected implementation, admitted host
operation, finite presentation resource, and exact display Base remain Plan and
Host truth. Transform Kinds do not acquire hidden manifestation side effects.

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
HOST  a running software environment
PART  the presence a host contributes to a body
CAPABILITY  truthful finite realization offer from a part
ROLE  semantic requirement declared by a form
CAST  exact binding of roles to capabilities
LINK  communication path between parts
BODY  durable top-level realization of a form
SOUL  durable continuity and recoverable identity of a body
```

A Host produces a Part. A Part may pair with a Body and offer capabilities. A Form contains configured Gears and may require Roles. A Cast binds Roles to exact capabilities. A Plan binds each Gear to exact implementation, Part, Host, Base, authority, resource, route, and bound facts. A Play starts that Plan. A Soul preserves continuity across restarts without pretending a restarted Boot is the same execution session.

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
8. domain proofs such as Tongues and Netherwick;
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
- BODY/PART/CAPABILITY/ROLE/CAST/LINK/SOUL;
- a real Host Observatory over authoritative reports;
- Copy a file as the first unfamiliar-user task;
- the Workbench as an authoring client of forms and the runtime;
- test receipts and resumable CI;
- the Pico W AP, DHCP, DNS, and HTTP appliance after the smaller Pico proof;
- Tongues as a speech/audio brownfield profile;
- Netherwick and Pete as describe-only, then HIL-safe robotics profiles;
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
