# Conduit Host Specification

**Document:** CHS-0  
**Status:** Foundational draft  
**Implementation issue:** [#347](../../issues/347)  
**Companion architecture:** [Portable Host Architecture](portable-hosts.md)  
**Audience:** Implementers, reviewers, maintainers, and coding agents

## 1. Purpose

This document specifies the Conduit **host**.

A host is the fundamental active component of a Conduit system. Every executable Conduit system is assembled from one or more hosts.

A host may realize capabilities directly through its software platform, or it may realize capabilities by composing other hosts.

A host produced through composition is itself a host and obeys the same external contract as any other host.

This recursive rule is the foundation of the architecture:

```text
Host := PrimitiveHost | CompositeHost(Form, ChildHosts, Plan)
```

The distinction between primitive and composite hosts is private to the host unless explicitly exposed for inspection.

A form using a host does not need to know how that host realizes its capabilities.

## 2. Normative language

The terms **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**, **MAY**, and **OPTIONAL** are normative.

An implementation conforms to this specification only when it satisfies all requirements marked MUST or MUST NOT for its declared host profile.

## 3. Architectural statement

A Conduit system consists of software hosts that offer semantic capabilities.

A form describes work in terms of those semantics.

A planner selects capabilities offered by particular host instances and creates an exact plan.

The selected hosts prepare the plan before any source begins producing values.

After preparation succeeds, the plan is started.

Values then travel through bounded, typed connections between host-resident operations.

The same semantic operation may have different physical manifestations on different hosts.

For example, the semantic operation:

```text
show Signal
```

may be manifested as:

- a line written to standard output;
- a visible browser element;
- an onboard LED;
- a display pixel;
- a sound;
- or another implementation that faithfully satisfies the same semantic contract.

The authored form does not select the manifestation.

The plan does.

## 4. Fundamental concepts

### 4.1 Host implementation

A **host implementation** is software capable of creating host instances.

Examples include:

- Pico W firmware;
- a generic Rust standard-library executable;
- a browser WebAssembly runtime;
- a mobile application runtime;
- a virtual host supervisor;
- a composite-host runtime.

A host implementation is comparable to a program or runtime class. It is not one particular running host.

### 4.2 Host instance

A **host instance** is one running or recoverable instance of a host implementation.

Each host instance has:

- a host identity;
- a boot identity;
- a declared host profile;
- a current lifecycle state;
- zero or more capability offers;
- zero or more prepared or active placements;
- zero or more links;
- and an observation record.

### 4.3 Platform

A **platform** is the environment used by a primitive host implementation.

Examples include:

- RP2040 and CYW43 hardware;
- Rust `std`;
- a browser and WebAssembly environment;
- an operating-system process;
- an embedded executor.

The platform is not itself the host.

The software presenting the Conduit host contract is the host.

### 4.4 Planning scope

A **planning scope** is the set of host instances currently visible to one planning scope.

A planning scope is not necessarily a permanent global object.

It may be:

- an explicit development session;
- a set of discovered peers;
- the child-host set of a composite host;
- the participating hosts of a durable body;
- or another bounded planning domain.

A planning scope describes visibility for planning. It does not, by itself, grant trust or execution authority.

### 4.5 Capability

A **capability** is a host's current offer to realize a semantic kind under explicit limits.

A capability offer states:

> This host can presently realize this meaning, using this implementation, within these limits.

A capability offer is not:

- an authority grant;
- an active gear;
- a plan;
- a promise of permanent availability;
- or clue that the capability has already been used.

### 4.6 Kind

A **kind** defines semantic meaning.

Examples include:

```text
flow/pulse
presentation/show
network/http
vision/frame
speech/synthesize
storage/value
```

A kind defines what an operation means and the rules an implementation must satisfy.

A kind MUST NOT require one particular platform merely because its first implementation uses that platform.

### 4.7 Implementation

An **implementation** is a concrete realization of a kind.

Example:

```text
presentation/show
    stdout/show-signal
    browser/show-signal
    pico-led/show-signal
```

These implementations may all satisfy the same kind. They differ in manifestation and platform mechanics, not in semantic purpose.

### 4.8 Form

A **form** is an authored semantic graph.

A form declares:

- operations;
- their kinds;
- typed inputs and outputs;
- connections;
- semantic configuration;
- constraints;
- and exposed boundaries.

A form MUST NOT require exact host identities, boot identities, implementation identities, transport addresses, device paths, or physical manifestations unless those facts are part of the form's intended meaning.

### 4.9 Plan

A **plan** is an immutable, exact realization of a form for a particular set of host instances and capability offers.

A plan selects:

- the exact source document, checked semantic form, and expanded graph;
- implementations;
- host placements;
- capability offers;
- connection bases;
- capacities;
- resource limits;
- initialization data;
- Play start order;
- authority requirements;
- and expected completion clue.

Creating a plan MUST NOT start it.

### 4.10 Active plan

An **active plan** is a prepared plan whose placements have been committed for execution.

The active plan remains inspectable until it completes, fails, is cancelled, is replaced, or the participating hosts become unrecoverably unavailable.

This specification does not require a separate public noun for active execution.

## 5. The two fundamental port directions

All Conduit dataflow is expressed using two fundamental port directions.

### 5.1 Input

An input consumes values of one declared type.

```text
Input<T>
```

An input has:

- a value kind;
- a queue or rendezvous policy;
- a finite capacity;
- pressure behavior;
- completion behavior;
- failure behavior;
- and authority requirements where applicable.

### 5.2 Output

An output produces values of one declared type.

```text
Output<T>
```

An output has:

- a value kind;
- ordering behavior;
- finite production constraints where applicable;
- pressure behavior;
- completion behavior;
- and failure behavior.

### 5.3 Derived operation shapes

A source has outputs and no required data inputs:

```text
Source<T> = Output<T>
```

A sink has inputs and no required data outputs:

```text
Sink<T> = Input<T>
```

A transformation has both:

```text
Transform<A, B> = Input<A> + Output<B>
```

A router, merger, accumulator, store, protocol adapter, planner, renderer, or controller is likewise described using combinations of typed inputs and outputs.

No additional fundamental dataflow direction is required.

Control operations such as prepare, start, cancel, and inspect belong to the host protocol rather than ordinary form dataflow.

## 6. Host identity

### 6.1 Host identity

Every host instance MUST have a `host_id`.

The host identity identifies the logical host across observations and, where supported, across restarts.

A host identity MUST NOT be derived solely from:

- an IP address;
- a process identifier;
- a browser tab;
- a USB path;
- a hostname;
- or a transient transport endpoint.

### 6.2 Boot identity

Every host start MUST create a fresh `boot_id`.

The pair:

```text
(host_id, boot_id)
```

identifies one exact running incarnation of a host.

A plan MUST identify both.

A host MUST reject a plan fragment addressed to an obsolete boot identity.

### 6.3 Implementation identity

A host MUST report its host implementation identity.

This identity SHOULD include enough information to distinguish:

- implementation name;
- protocol version;
- build or artifact digest;
- host profile;
- and compatibility level.

### 6.4 Composite host identity

A composite host has its own host identity.

Its identity is not merely the set of its child host identities.

A composite host MAY replace, add, remove, or replan child hosts while preserving its external host identity, provided it continues to satisfy its advertised contracts and continuity policy.

## 7. Host profiles

Every host declares one or more conformance profiles.

### 7.1 Core profile

Every host MUST implement the core profile.

The core profile requires:

- identity;
- lifecycle reporting;
- capability advertisement;
- plan-fragment validation;
- preparation;
- Play start;
- cancellation;
- cleanup;
- bounded connections;
- and machine-readable observations.

### 7.2 Rust standard-library profile

A Rust standard-library host:

- uses portable Rust `std`;
- MUST NOT define itself in terms of Linux-only APIs;
- may run on Linux, Windows, macOS, BSD, or another supported system;
- may use standard input and output;
- may use threads;
- may use TCP and UDP;
- may use the filesystem;
- and may implement planning and operator functions.

### 7.3 Browser profile

A browser host:

- runs within a browser execution environment;
- may use WebAssembly;
- MUST have an identity separate from the browser tab;
- MUST permit multiple independent host instances in one page or browser application;
- MUST keep host state separate from DOM presentation state;
- and MAY use browser-local, worker, message-channel, or WebSocket links.

A browser page is not automatically one host.

One page may contain several browser hosts, each with a distinct host identity and boot identity.

### 7.4 Pico W profile

A Pico W host:

- operates without Rust `std`;
- uses statically or explicitly bounded storage;
- MUST NOT require unbounded allocation;
- may offer timer, radio, GPIO, LED, UART, and other initialized capabilities;
- MUST report unavailable peripherals honestly;
- and MUST retain machine-readable observations independently of physical manifestations such as LED state.

### 7.5 Composite profile

A composite host realizes some or all of its capabilities through child hosts.

A composite host MUST satisfy every external requirement of the core profile.

A parent planner MUST NOT need to understand the composite host's internal topology to use its advertised capabilities.

## 8. Host lifecycle

A host session has the following conceptual lifecycle states:

```text
starting
available
draining
stopped
faulted
```

### 8.1 Starting

The host is initializing its identity, platform adapters, capability implementations, and inspection service.

It MUST NOT advertise a capability that is not yet initialized.

### 8.2 Available

The host may advertise capabilities, accept plan fragments, prepare placements, execute active placements, create links, and report observations.

### 8.3 Draining

The host accepts no new placements.

It may complete, cancel, or clean up existing placements.

### 8.4 Stopped

The host is no longer active.

Its boot identity is no longer valid.

### 8.5 Faulted

The host cannot satisfy the core host contract.

A faulted host MUST NOT silently remain available.

It MUST expose the fault where communication remains possible.

## 9. Placement lifecycle

Host lifecycle and placement lifecycle are separate.

A host may operate many placements concurrently while remaining available.

Each placement progresses through states comparable to:

```text
proposed
reserved
prepared
active
completed
failed
cancelled
released
```

### 9.1 Proposed

A plan fragment names the proposed placement.

No resources are yet promised.

### 9.2 Reserved

The host has reserved required local resources.

No semantic source may begin producing solely because reservation succeeded.

### 9.3 Prepared

The placement's implementation, inputs, outputs, links, resources, and authority have been validated and prepared.

A prepared placement MUST be able either to commit or to release its reservation.

### 9.4 Active

The plan has been committed.

The placement may consume inputs, produce outputs, and create effects permitted by the plan.

### 9.5 Completed

The placement reached its specified terminal condition successfully.

### 9.6 Failed

The placement reached a defined failure.

The failure MUST include machine-readable disposition information.

### 9.7 Cancelled

Execution was intentionally stopped before normal completion.

### 9.8 Released

All resources owned by the placement have been released or transferred according to an explicit continuation rule.

## 10. Capability advertisement

A host MUST publish a bounded capability advertisement.

Conceptually:

```rust
struct HostAdvertisement {
    protocol_version: ProtocolVersion,
    host_id: HostId,
    boot_id: BootId,
    host_profile: HostProfileId,
    offer_generation: u64,
    capabilities: BoundedList<CapabilityOffer>,
}
```

A capability offer is conceptually:

```rust
struct CapabilityOffer {
    capability_id: CapabilityId,
    kind_id: KindId,
    kind_contract_revision: KindContractRevision,
    execution_profile_id: ExecutionProfileId,
    implementation_id: ImplementationId,
    inputs: BoundedList<PortDescription>,
    outputs: BoundedList<PortDescription>,
    limits: CapabilityLimits,
    restrictions: CapabilityRestrictions,
    manifestation: ManifestationDescription,
}
```

The kind contract revision owns the exact ordered input and output contracts.
An offer is compatible only when the advertised revision and every port agree
with the checked Gear; queue limits do not stand in for type or port
compatibility.

### 10.1 Offer generation

Advertisements MUST include a monotonically changing generation or equivalent freshness mechanism.

A plan MUST pin the exact offer generation on which it depends.

A host MUST reject preparation when a required offer is no longer valid.

### 10.2 Required distinctions

A host and planner MUST distinguish:

```text
known kind
installed implementation
initialized capability
advertised capability
reserved capability
planned placement
active placement
```

These states MUST NOT be collapsed into one boolean.

### 10.3 Limits

A capability offer MUST state relevant finite limits.

These may include:

- maximum active instances;
- maximum input queue items;
- maximum output queue items;
- maximum encoded value size;
- maximum fan-out;
- maximum rate;
- memory budget;
- session count;
- timing precision;
- supported link bases;
- and platform-specific restrictions.

### 10.4 Manifestation

A capability MAY describe its manifestation.

The manifestation description explains how the semantic operation becomes perceptible or effective on this host.

For example:

```text
kind: presentation/show<Signal>
manifestation: stdout line
```

or:

```text
kind: presentation/show<Signal>
manifestation: onboard LED
```

The manifestation does not change the kind.

## 11. Plan fragments

A host receives only the fragment of a plan assigned to it.

A plan fragment MUST contain enough information to execute and inspect the assigned work without consulting mutable authored source.

It MUST identify:

- plan identity;
- exact host identity;
- exact boot identity;
- exact offer generation;
- assigned Gears;
- selected implementations;
- capability bindings;
- typed inputs and outputs;
- local connections;
- remote connection endpoints;
- capacities;
- initialization values;
- startup dependencies;
- authority material or references;
- terminal conditions;
- required observations;
- and cleanup behavior.

A host MUST reject a fragment that is:

- addressed to another host;
- addressed to an obsolete boot;
- based on stale capability offers;
- above declared limits;
- incompatible with local kinds;
- malformed;
- unauthorized;
- or internally inconsistent.

## 12. Preparation and Play start

### 12.1 Separation

Preparation and Play start MUST be separate operations.

Preparation may allocate resources and establish links.

Preparation MUST NOT begin normal source production or external effects.

### 12.2 Required preparation sequence

A host preparing a plan fragment MUST:

1. validate the plan identity;
2. validate its target host and boot identities;
3. validate pinned capability offers;
4. validate resource bounds;
5. validate kinds and implementations;
6. reserve local resources;
7. construct assigned operation instances;
8. establish or accept required connections;
9. verify authority;
10. report prepared status or a precise rejection.

### 12.3 Play start

A host starts a prepared fragment only after receiving a valid commit for the exact plan.

Play start MUST be idempotent or MUST reject duplicate commits unambiguously.

### 12.4 Failed preparation

If any required host fails to prepare, the planner or coordinating host MUST NOT start the plan.

Every host that prepared successfully MUST be instructed to release its prepared fragment.

### 12.5 Failure after Play start

A failure after Play start MUST NOT be rewritten as successful completion.

The host MUST report:

- the failing placement;
- the failure class;
- the last accepted or produced sequence where relevant;
- the disposition of buffered values;
- and whether external effects may already have occurred.

## 13. Connections

### 13.1 Semantic connection

A connection carries typed values from one output to one input.

Its semantic contract does not depend on the underlying transport.

The same connection may be realized as:

- an in-process queue;
- a static embedded queue;
- a browser message channel;
- shared memory;
- WebSocket;
- TCP;
- UDP with an appropriate reliability profile;
- serial;
- or a composite-host internal route.

### 13.2 Boundedness

Every connection MUST have explicit finite bounds.

At minimum, a plan MUST define:

- maximum queued values;
- maximum encoded bytes;
- pressure behavior;
- ordering behavior;
- and terminal behavior.

No host or link base may silently insert an unbounded queue.

### 13.3 Pressure

When a consumer cannot accept another value, the connection MUST apply its declared pressure rule.

The first Conduit profile SHOULD use backpressure rather than silent dropping.

Other policies MAY later include reject, drop newest, drop oldest, sample, conflate, or fail.

Such behavior MUST be explicit in the kind or plan.

### 13.4 Fan-out

One output feeding multiple inputs MUST be realized as multiple independently bounded connections unless a declared multicast kind specifies otherwise.

### 13.5 Wire envelope

Remote links MUST use a versioned bounded envelope.

Conceptually:

```rust
struct ConnectionEnvelope {
    protocol_version: u16,
    plan_id: PlanId,
    connection_id: ConnectionId,
    sequence: u64,
    value_kind: KindId,
    payload: BoundedBytes,
}
```

Malformed or oversized envelopes MUST be rejected before unbounded allocation or semantic execution.

## 14. Observation and Clues

A host MUST provide machine-readable observations.

Visual or physical manifestation alone is insufficient clue.

For example, a Pico W LED may visibly blink, but the host must also retain or emit records such as:

```text
plan: signal-demo-17
placement: show-3
sequence: 8
level: false
result: manifested
```

Required observation classes include:

- host start;
- capability advertisement;
- capability withdrawal;
- plan-fragment receipt;
- preparation success;
- preparation rejection;
- Play start;
- value receipt;
- value production;
- semantic manifestation;
- completion;
- failure;
- cancellation;
- and resource release.

Observation storage MUST be bounded or explicitly streamed.

## 15. Primitive hosts

A primitive host realizes capabilities directly from its platform.

Examples:

```text
Rust std host
    presentation/show<Signal> -> stdout

Browser host
    presentation/show<Signal> -> DOM

Pico W host
    presentation/show<Signal> -> onboard LED
```

A primitive host may still contain internal libraries, drivers, tasks, workers, or executors.

Primitive means only that the host does not expose those internal components as child Conduit hosts for the capability in question.

Primitive does not mean simple.

## 16. Composite hosts

### 16.1 Definition

A composite host is a host that realizes one or more capabilities through a planning scope of child hosts and an internal plan.

Conceptually:

```text
CompositeHost {
    host identity
    external capability offers
    child host planning scope
    internal form
    internal plan
    internal active state
    boundary mappings
}
```

### 16.2 Closure under composition

The result of composition MUST itself satisfy the host contract.

Therefore:

```text
compose(host A, host B, form F) -> host C
```

Host C may then participate in another planning scope:

```text
compose(host C, host D, form G) -> host E
```

No parent form needs to know that C is composite.

### 16.3 External boundary

A composite host advertises only capabilities exposed through its external boundary.

Internal operations, child identities, queues, and links are private unless inspection policy exposes them.

An external input maps to one or more internal inputs.

An external output maps from one or more internal outputs.

An external capability may represent a complete internal graph.

### 16.4 Example

Suppose two child hosts offer:

```text
host A:
    flow/pulse -> Output<Signal>

host B:
    presentation/show -> Input<Signal>
```

An internal form connects them:

```text
pulse > show
```

The resulting composite host might expose:

```text
capability: demonstration/run-signal
```

or it might expose the pulse output, show input, completion clue, or another declared boundary.

The parent planning scope sees one host offering the exposed capability.

It need not place the internal pulse and show operations itself.

### 16.5 Internal replanning

A composite host MAY replace its internal plan while preserving external identity.

It may do so only when:

- its advertised external kinds remain satisfied;
- external ordering and state guarantees remain valid;
- active parent-plan commitments remain honored;
- and the change is reflected in observations.

### 16.6 Child ownership

In the baseline profile, one exact capability instance MUST NOT be controlled concurrently by conflicting parent plans.

A child host MAY participate in several composite hosts only when it exposes isolated capability instances with separate resource and authority boundaries.

### 16.7 Failure transparency

A composite host MUST translate internal failure into an externally valid failure.

It MUST NOT claim success merely because its external process remained alive.

It MAY expose internal diagnostics, but it MUST first preserve the semantic failure contract visible to its parent.

## 17. The planner as a host capability

Planning is not required to be a permanent central service.

A host may offer a planning capability.

For example:

```text
Input<Form>
Input<HostAdvertisementSet>
Input<PlacementPolicy>
Output<Plan>
Output<PlanningDiagnostic>
```

The first implementation may place this capability on the Rust standard-library host.

A later composite host may realize planning through several internal hosts.

Forms, capability advertisements, plans, and diagnostics can therefore participate in the same host-composition model as other data.

## 18. A planning scope as a composite host

A planning scope may be wrapped and exposed as a composite host.

This is the primary recursive construction.

Internally:

```text
planning scope
    host A
    host B
    host C
    planner
    active plans
```

Externally:

```text
composite host R
    capability X
    capability Y
    capability Z
```

This allows an entire robot, service, browser cluster, appliance, or distributed application to appear as one host in a larger system.

A durable body may eventually be represented as a composite host whose child hosts change while its external identity remains stable.

The `.soul` layer may preserve the identity and continuity of that composite host.

Those durable semantics are outside the baseline host protocol but MUST remain compatible with it.

## 19. First portable semantic proof

The first required value is:

```rust
struct Signal {
    sequence: u64,
    level: bool,
}
```

The first source kind is:

```text
flow/pulse -> Output<Signal>
```

The first sink kind is:

```text
presentation/show <- Input<Signal>
```

The semantic requirement of `presentation/show` is:

1. accept signals in increasing sequence order;
2. manifest the exact `level`;
3. retain or emit an observation containing the exact sequence and level;
4. report failure when manifestation cannot be completed.

Required manifestations are:

```text
Rust std host:
    false -> stdout "off"
    true  -> stdout "on"

Browser host:
    false -> visible indicator off
    true  -> visible indicator on

Pico W host:
    false -> onboard LED off
    true  -> onboard LED on
```

The form remains:

```conduit
signal-demo {
    pulse: flow/pulse
    show: presentation/show

    pulse.count = 16
    pulse.period-ms = 250
    pulse.initial = false

    pulse > show
}
```

The form contains no reference to stdout, DOM, GPIO, Pico W, browser, operating system, WebSocket, TCP, or host identity.

## 20. Conformance invariants

Every conforming implementation MUST preserve these distinctions:

```text
A platform is not a host.

A host implementation is not a host instance.

A host identity is not a boot identity.

A known implementation is not an initialized capability.

An initialized capability is not necessarily advertised.

An advertisement is not authority.

A capability offer is not a placement.

A placement is not active merely because it is planned.

A plan is not active merely because it is prepared.

A semantic kind is not its manifestation.

A remote connection is not its transport.

A visual effect is not sufficient execution clue.

A composite host is still a host.
```

## 21. Minimum host conformance tests

A core host implementation MUST demonstrate:

1. creation of a unique host and boot identity;
2. a bounded capability advertisement;
3. rejection of a stale boot identity;
4. rejection of a stale capability generation;
5. successful preparation without premature Play start;
6. successful Play start after commit;
7. bounded input and output connections;
8. correct pressure behavior;
9. deterministic completion or explicit failure;
10. machine-readable observations;
11. cleanup of released placements;
12. and honest withdrawal of unavailable capabilities.

A composite host MUST additionally demonstrate:

1. at least two child hosts;
2. one internal form;
3. one internal exact plan;
4. one externally advertised capability;
5. use of that capability by a parent plan;
6. preservation of boundedness across the boundary;
7. translation of internal failure to external failure;
8. and continued conformance when inspected only as a host.

## 22. Deliberate non-requirements

This specification does not yet require:

- durable body identity;
- `.soul` archival recovery;
- automatic peer discovery;
- cryptographic admission;
- consensus;
- transparent migration of arbitrary state;
- global scheduling optimization;
- generalized retries;
- transparent reconnection;
- multi-parent capability leasing;
- or a universal wire transport.

These features may be added without changing the central host contract.

## 23. Questions requiring later resolution

The following questions remain open:

1. Whether `planning scope` remains a permanent public term or only a planning view.
2. Whether every composite host must have one parent at a time.
3. How host identity persists across erased or replaced storage.
4. How authority is delegated into and out of composite hosts.
5. Whether capability advertisements are snapshots or an ordered event stream.
6. How an active plan responds when a capability offer is withdrawn.
7. Which internal composite-host details are visible by default.
8. Whether a composite host may preserve external identity while replacing every child.
9. How long-running state is checkpointed during internal replanning.
10. How durable bodies and `.soul` archives fence duplicate restorations.
11. Whether the planner is always modeled as an ordinary host capability.
12. Whether nested composition is observationally associative.
13. How capability quality and manifestation fidelity are ranked.
14. How semantic transformations between unequal manifestations are represented.
15. Whether one host may partition itself into independently leasable subhosts.

## 24. Summary

The Conduit host is a software runtime that offers bounded semantic capabilities, accepts exact plan fragments, prepares them without starting them, starts them only after commitment, carries typed values through bounded connections, and produces machine-readable clue.

A primitive host realizes capabilities from a platform.

A composite host realizes capabilities from child hosts.

The two are externally equivalent.

Therefore the complete architecture can be constructed recursively from hosts:

```text
hosts offer capabilities

forms describe semantic work

plans bind work to hosts

active plans perform the work

composed hosts hide internal plans
and offer new capabilities

those hosts may be composed again
```

The architectural closure is:

```text
Host + Host + Form + Plan -> Host
```

That closure is the system.
