# Portable Host Architecture

**Status:** Foundational architecture for the Conduit reboot  
**Implementation issue:** [#347](../../issues/347)  
**Audience:** Contributors, maintainers, coding agents, and reviewers

## Summary

Conduit lets an authored form run across different software hosts without baking those hosts into the form.

A realm contains host instances. Each host advertises the semantic capabilities it can currently provide. A form describes work in terms of those semantics. A planner combines the form, the realm's current capability offers, explicit policy, and available links to create an exact plan. Activating that plan starts the work only after every required placement and connection is ready.

The same semantic operation may manifest differently on different hosts.

For the first complete proof:

- a Rust standard-library host manifests `show(Signal)` through stdout;
- a browser host manifests `show(Signal)` through a visible DOM indicator;
- a Pico W host manifests `show(Signal)` through its onboard LED.

The form does not name stdout, the DOM, GPIO, Linux, a browser tab, an IP address, or a device path. It says only that a signal must be shown.

This is the center of the reboot:

> Forms describe meaning. Hosts offer implementations of that meaning. Plans make the mapping exact.

## Why this architecture exists

Traditional distributed systems usually make platform decisions early. A graph node may be called `write_stdout`, `set_gpio`, `update_dom`, or `serve_http`. Once those names enter the authored graph, the graph has already chosen its environment.

Conduit instead keeps three questions separate:

1. **What should happen?**
2. **What can the current hosts do?**
3. **How will this particular realm realize the work now?**

A form answers the first question.

Capability advertisements answer the second.

A plan answers the third.

This separation lets one form move among a microcontroller, a browser, a desktop process, and later a distributed robot without pretending those environments are identical.

They are not identical. Their implementations, limits, links, clocks, storage, and failure modes differ. Conduit preserves those differences in capability advertisements and plans while keeping the authored meaning portable.

## Architectural vocabulary

This document uses ordinary engineering language. These are concepts, not a requirement that every public command or file format use the same words forever.

### Realm

A realm is the set of host instances currently visible to one planning session.

For the first implementation, realm membership may be explicit and development-oriented. A host can register with the local operator or planner and become available for placement.

A realm records or observes:

- host identities;
- host boot or session identities;
- current capability advertisements;
- available links among hosts;
- exact plans;
- and active plan state.

A realm is not merely a network segment. A reachable endpoint is not automatically a host, and a host is not automatically trusted merely because it is reachable.

Durable body identity, secure admission, recovery, and `.soul` archives are later layers described near the end of this document. They must build on this model rather than distort the first implementation.

### Host

A host is a software runtime that can execute cells and advertise capabilities.

The host is not the physical machine by itself.

Examples:

- firmware running on a Pico W;
- a portable Rust process using `std`;
- a browser runtime compiled to WebAssembly;
- a future Android runtime;
- a future ConduitOS runtime.

A host is responsible for:

- assigning or restoring its host identity;
- assigning a fresh boot or session identity;
- discovering which implementations are currently initialized;
- advertising capabilities and their limits;
- reserving cell instances and queues for a plan;
- executing selected cells;
- carrying local and remote cords;
- reporting receipts, completion, and failure;
- and releasing resources when a plan ends or is replaced.

A host advertisement must distinguish the durable host identity from the current boot identity. A plan made against a previous boot must not silently attach itself to a restarted host.

### Capability

A capability is a current offer by a host to realize a semantic kind under explicit limits.

A capability advertisement says something like:

> This host can currently realize `display/show<Signal>` using implementation `pico/onboard-led-v1`, for at most one active instance, with a queue capacity no greater than four values.

A capability is more precise than a feature flag. It should identify:

- the semantic kind;
- the exact implementation;
- the host and boot that offer it;
- supported value kinds;
- instance limits;
- queue and byte limits;
- configuration restrictions;
- relevant resource identity;
- and any important failure constraints.

The architecture must preserve the following distinctions:

```text
A kind is known.
An implementation is installed.
An implementation is initialized.
A capability is currently advertised.
A planner selected the capability.
A plan reserved the capability.
The capability is active.
```

These are different states. They must not collapse into one boolean.

Advertising a capability is an observation, not an authority grant and not an instruction to start work.

### Kind

A kind is a semantic contract.

A kind defines what a cell means, what typed values it accepts or produces, how it completes, how it fails, and what behavior an implementation must preserve.

Examples used in the first slice:

```text
flow/pulse
presentation/show
value/signal
```

Kinds do not name platforms.

`presentation/show` is not `stdout`, `set_led`, or `update_dom`. Those are implementations or manifestations of the same semantic request on particular hosts.

### Implementation

An implementation is platform-specific code that realizes a kind.

Examples:

```text
std/pulse-v1
browser/pulse-v1
pico/pulse-v1

std/stdout-show-signal-v1
browser/dom-show-signal-v1
pico/onboard-led-show-signal-v1
```

An implementation may have narrower limits than the kind permits. The capability advertisement exposes those limits to the planner.

Several implementations may realize one kind. One host may advertise several implementations of the same kind.

### Manifestation

A manifestation is the observable host-specific expression of a semantic operation.

For `presentation/show<Signal>`:

- stdout text is the standard host manifestation;
- a visible DOM indicator is the browser manifestation;
- the onboard LED state is the Pico W manifestation.

A richer host may add useful presentation, but it must preserve the semantic value.

For the first `Signal` profile:

```text
false means off
true means on
```

The standard host may print `off` and `on`. The browser may show text and a lamp graphic. The Pico W may set a physical LED. All three are faithful because they preserve the exact boolean level.

This is why the first demonstration uses a signal instead of arbitrary text. A single LED cannot faithfully display arbitrary prose without adding another transformation such as Morse encoding. That would test two semantic operations at once and blur the architectural proof.

### Form

A form is an authored semantic graph.

A form names:

- cells;
- their kinds;
- typed cords;
- semantic configuration;
- finite bounds;
- and requirements or constraints that genuinely belong to the work.

A form does not name:

- exact host instances;
- host boot identifiers;
- operating systems;
- browser element identifiers;
- GPIO pins;
- stdout;
- sockets;
- IP addresses;
- WebSocket URLs;
- device paths;
- or implementation identifiers.

The same form should remain valid when the planner chooses a different faithful realization.

### Cell

A cell is one named occurrence in a form.

A cell has a semantic kind. A plan later selects an implementation and host placement for that occurrence.

The first architecture needs only a small number of structural cell roles. It should resist a rush toward a huge standard library.

### Source and sink

The two fundamental structural cell shapes are:

```text
Source<T>  produces typed values
Sink<T>    consumes typed values
```

These are not hardware-specific kinds. They are the smallest composable shapes needed to establish a typed flow.

A source is responsible for:

- producing values of one declared kind;
- respecting configured finite bounds;
- obeying downstream pressure;
- completing explicitly;
- and reporting failure rather than hiding it.

A sink is responsible for:

- accepting values of one declared kind;
- preserving their required ordering;
- performing its semantic effect;
- producing evidence or receipts when required;
- completing explicitly;
- and reporting failure rather than pretending success.

Transformations will later consume and produce values, effectively composing a sink side with a source side. The first proof does not need a third fundamental shape to establish portability.

### Cord

A cord carries typed values between cells.

The semantic meaning of a cord does not depend on whether the selected plan realizes it as:

- an in-process bounded queue;
- an in-memory connection between browser host instances;
- a WebSocket;
- TCP;
- UDP with an explicit reliability profile;
- shared memory;
- or a future physical transport.

Transport independence does not mean transport differences disappear. The exact plan records the provider, limits, ordering guarantees, framing, and failure behavior.

### Plan

A plan is an immutable, exact realization of a form for one realm state.

A plan fixes:

- the exact form digest;
- every cell identity;
- every selected implementation;
- every host identity and boot identity;
- every selected capability advertisement;
- every cord realization;
- every queue item limit;
- every byte limit;
- finite source bounds;
- startup dependencies;
- expected receipts;
- terminal behavior;
- and all other choices needed for execution.

A plan is not a suggestion. It is the complete answer to how the form will run in this realm.

A plan is not active merely because it exists.

Any change to placement, implementation, transport, bounds, or required capabilities creates a new plan.

### Activation

Activation prepares and starts a plan.

The source must not begin emitting merely because planning succeeded. Every required sink, queue, and remote link must be prepared first.

The minimum activation sequence is:

1. Resolve every selected host by exact host ID and boot ID.
2. Confirm every selected capability advertisement is still current.
3. Reserve every cell instance.
4. Reserve every bounded queue and buffer.
5. Establish every remote link.
6. Confirm every required sink is ready.
7. Start finite sources.
8. Carry values until completion or failure.
9. Collect required receipts.
10. Release resources or retain explicitly declared state.

If preparation fails, no source begins.

If execution fails after activation, the plan must expose the failure and account for values already delivered and values not delivered. The first implementation does not need transparent reconnection or replay.

### Receipt

A receipt is machine-readable evidence that an expected semantic event occurred.

For the first `show(Signal)` sink, a receipt records the exact sequence and level that the host implementation successfully manifested.

Receipts matter because human observation is not sufficient evidence:

- seeing an LED blink does not prove which sequence value it represented;
- seeing browser text does not prove the runtime accepted the correct envelope;
- seeing stdout does not prove all three sinks received identical values.

The decisive three-host test compares receipts from every sink.

## The first portable value

The first value is deliberately tiny:

```rust
pub struct Signal {
    pub sequence: u64,
    pub level: bool,
}
```

The initial source emits sixteen signals.

```text
count = 16
period_ms = 250
initial = false
```

The sequence begins at zero and increases monotonically.

The level alternates:

```text
(0, false)
(1, true)
(2, false)
...
(15, true)
```

No timestamp is required in the first profile. Timestamps would immediately introduce clock source, resolution, synchronization, and interpretation questions that are irrelevant to the first portability proof.

## The first source

The first source kind is `flow/pulse`.

It is a finite `Source<Signal>`.

Configuration:

```text
count
period_ms
initial
```

Its contract is:

- emit exactly `count` values unless it fails or is cancelled;
- begin at sequence zero;
- increment sequence by one for each value;
- alternate level beginning with `initial`;
- wait approximately `period_ms` according to the selected implementation's declared timer behavior;
- respect cord pressure;
- and complete after the final value.

It must not create an unbounded task, unbounded queue, or ambient callback stream.

## The first sink

The first sink kind is `presentation/show` specialized for `Signal`.

It is a `Sink<Signal>`.

Its contract is:

1. accept signals in increasing sequence order;
2. manifest the exact `level` value through the selected host implementation;
3. produce a receipt containing the exact sequence and level after successful manifestation;
4. never claim success before the host-specific operation completes according to its contract;
5. complete after the upstream source completes and all accepted values are accounted for.

The sink's meaning is stable even though the manifestation changes.

## The first forms

### Portable pair

One unchanged form must work with any valid placement:

```conduit
form 0

signal-demo {
    pulse: flow/pulse
    show: presentation/show

    pulse.count = 16
    pulse.period-ms = 250
    pulse.initial = false

    pulse > show
}
```

The exact source grammar is still subject to implementation, but the semantic content must remain this small.

The form contains no platform facts.

### Three manifestations

The decisive realm demonstration uses one source and three instances of the same sink kind:

```conduit
form 0

triple-signal {
    pulse: flow/pulse
    local: presentation/show
    web: presentation/show
    light: presentation/show

    pulse.count = 16
    pulse.period-ms = 250
    pulse.initial = false

    pulse > local
    pulse > web
    pulse > light
}
```

Fan-out is compiled into three independently bounded cords.

The first planner may accept explicit operator placement:

```text
pulse -> Rust std host
local -> Rust std host stdout capability
web   -> selected browser host DOM capability
light -> Pico W onboard LED capability
```

Automatic placement ranking is not required for the first issue.

## The three initial host platforms

### Browser host

The browser host proves that a host is a software participant, not a physical machine or browser tab.

One page must be able to create several independent browser host instances, such as host A, host B, and host C.

Each instance has its own:

- host ID;
- boot ID;
- capability advertisements;
- reserved cell instances;
- queues;
- receipts;
- and execution state.

The browser host must:

- share the host-neutral protocol and semantic value definitions;
- advertise `flow/pulse` and `presentation/show<Signal>` where supported;
- realize `show` through a visible indicator plus exact textual sequence and level;
- support a bounded in-memory link between browser host instances;
- support a bounded WebSocket link to the Rust standard-library host;
- keep runtime state separate from DOM presentation state;
- and provide a compact operator view of host instances, capabilities, placements, and receipts.

The DOM is a manifestation surface. It is not execution authority and must not secretly become the source of runtime truth.

The browser test policy is intentionally modest:

- compile the browser host in CI;
- test shared protocol, codec, planning, and queue behavior outside visual browser timing where possible;
- use deterministic unit tests for host state machines;
- allow a documented manual smoke test for the final visible presentation;
- do not recreate Playwright, browser retries, screenshot assertions, or a Chromium/Firefox/WebKit blocking matrix.

### Pico W host

The Pico W host proves that the architecture works under real `no_std`, memory, and device constraints.

It must:

- run on RP2040/Pico W without Rust `std`;
- use bounded static storage or explicitly bounded allocation;
- advertise only capabilities whose devices and supporting services initialized successfully;
- realize `presentation/show<Signal>` through the onboard LED;
- provide a finite timer implementation for `flow/pulse`;
- support one bounded remote link to the Rust standard-library host;
- retain exact machine-readable receipts independently of the visible LED;
- report unavailable radio, timer, or LED resources honestly;
- and provide one documented build-and-flash command.

The first Pico W slice does not require DHCP, DNS, captive portal behavior, HTTP serving, robot admission, serial motor control, or durable identity recovery.

Those may follow once the semantic source-to-sink path is proven.

### Rust standard-library host

The standard-library host is the generic hosted implementation.

It is not a Linux host. It must avoid Linux-specific APIs and claims unless a later optional capability explicitly requires them.

It should compile on ordinary Rust `std` platforms such as Linux, macOS, Windows, and BSD where dependencies permit.

It must:

- provide a command-line host process;
- advertise `flow/pulse` and `presentation/show<Signal>`;
- realize `show` through stdout;
- provide a monotonic timer implementation;
- support bounded WebSocket links to browser hosts;
- support one bounded TCP or UDP link to the Pico W host;
- and initially act as the operator and planning process for the demonstration realm.

The std host may temporarily provide explicit registration and rendezvous for development. That is a convenience for the first vertical slice, not a permanent central-coordinator requirement.

## Capability advertisements

The host-neutral model should resemble:

```rust
pub struct HostAdvertisement {
    pub protocol_version: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub profile: HostProfileId,
    pub capabilities: BoundedCapabilities,
}

pub struct CapabilityAdvertisement {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub implementation_id: ImplementationId,
    pub limits: CapabilityLimits,
}
```

The actual containers must be explicitly bounded where required, especially in shared core and Pico W code.

The first `CapabilityLimits` must express at least:

- supported value kind;
- maximum active cell instances;
- maximum queue items;
- maximum encoded item size;
- and any platform-specific restrictions relevant to planning.

Advertisements should be deterministic and inspectable. Agents should not add ambient discovery magic before the explicit model works.

## Planning

The first planner may be simple and explicit. It does not need optimization, backtracking, or policy synthesis.

Inputs:

```text
form
current host advertisements
current link observations
operator placement choices
fixed first-profile policy
```

Output:

```text
one immutable exact plan
```

The planner must reject invalid requests precisely.

Required rejection categories include:

- no matching source capability;
- no matching show capability;
- incompatible value kind;
- selected capability no longer advertised;
- stale host boot ID;
- required queue larger than a host limit;
- no supported link between selected hosts;
- duplicate or contradictory placement;
- and malformed form bounds.

A planner error should identify the cell, required kind, rejected candidates, and concrete reason. `No plan found` is not sufficient when more exact evidence is available.

## Exact plan model

A plan should contain enough information that a host never has to guess what the planner intended.

At minimum:

```rust
pub struct Plan {
    pub plan_id: PlanId,
    pub form_digest: FormDigest,
    pub hosts: BoundedHostSelections,
    pub cells: BoundedCellPlacements,
    pub cords: BoundedCordRealizations,
    pub startup: BoundedStartupSteps,
    pub expected_receipts: BoundedReceiptRequirements,
}
```

Each cell placement identifies:

- cell ID;
- semantic kind;
- implementation ID;
- host ID;
- boot ID;
- capability ID;
- configuration;
- and resource limits.

Each cord realization identifies:

- cord ID;
- value kind;
- writer placement;
- reader placement;
- provider;
- item capacity;
- encoded byte capacity;
- ordering profile;
- and terminal behavior.

The plan should be hashable or otherwise assigned a stable identity based on canonical contents.

## Cords and transports

The first cord profile is intentionally narrow:

```text
one writer
one reader per realized cord
ordered delivery
capacity = 4 values
no hidden unbounded buffering
no transparent reconnect
```

The three-manifestation form creates three cords rather than one magical broadcast channel.

Required initial providers:

```text
local             cells on one host
browser-memory    browser host instance to browser host instance
websocket         browser host to std host
tcp-or-udp        std host to Pico W host
```

Provider APIs must report a common set of outcomes:

```text
ready
delivered
full
disconnected
malformed
terminal
```

A provider may have additional diagnostics, but it must not erase these semantic outcomes.

No provider may rely on a hidden unbounded queue in a library, browser API wrapper, task channel, or socket adapter.

## Wire envelope

Remote cords need one compact, versioned envelope conceptually like:

```rust
pub struct CordEnvelope {
    pub protocol_version: u16,
    pub plan_id: PlanId,
    pub cord_id: CordId,
    pub sequence: u64,
    pub kind_id: KindId,
    pub payload: BoundedBytes,
}
```

The relationship between envelope sequence and value sequence must be explicit.

For the first profile, prefer one source of sequence truth. Either:

- the `Signal.sequence` field is canonical and the envelope sequence mirrors it with an invariant; or
- the envelope sequence is canonical and the decoded `Signal` contains only `level`.

Do not carry two unrelated sequence counters.

The encoding must:

- work in `no_std`;
- work in browser WebAssembly;
- have deterministic maximum sizes;
- reject malformed or oversized frames before expensive allocation or execution;
- and use shared test vectors across all three hosts.

A compact established encoding such as `postcard` is preferable to building a general serialization framework in this issue.

## Backpressure and boundedness

Boundedness is architectural, not a later optimization.

Every source, sink, queue, cord, advertisement, plan section, receipt collection, and transport frame must have an explicit bound or finite lifecycle in the first profile.

For the pulse demo:

- the source emits exactly sixteen values;
- each cord holds at most four values;
- each signal and envelope have a fixed maximum encoded size;
- every sink produces exactly sixteen receipts on success;
- and execution ends.

When a queue is full, the source must observe pressure. It may wait, yield, or fail according to the contract. It must not create another unbounded buffer to make the pressure disappear.

This requirement keeps the same semantics credible on the Pico W and prevents desktop and browser implementations from hiding invalid assumptions behind abundant memory.

## Receipts and evidence

Every successful show sink produces receipts equivalent to:

```rust
pub struct ShowReceipt {
    pub plan_id: PlanId,
    pub cell_id: CellId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub sequence: u64,
    pub level: bool,
    pub manifestation: ManifestationId,
}
```

The manifestation identifier distinguishes stdout, browser DOM, and Pico LED implementations without changing the semantic value.

The decisive proof compares the semantic receipt sequence from all three sinks:

```text
stdout receipts == browser receipts == Pico receipts
```

This is stronger than comparing logs or watching the UI.

## Required demonstrations

The implementation issue is complete only when all of these work.

### 1. Local standard host

```text
flow/pulse -> presentation/show
```

Both cells run on one Rust std host. Stdout and receipts account for sixteen signals.

### 2. Multiple browser hosts

One browser page creates at least two independent host instances.

The pulse source runs on browser host A. The show sink runs on browser host B. A bounded in-memory link carries the values.

The operator view visibly distinguishes both host and boot identities.

### 3. Local Pico W

Both cells run on the Pico W. The onboard LED alternates sixteen times and the host retains sixteen exact receipts.

### 4. Standard host to browser

The source runs on the Rust std host. The show sink runs on a browser host over WebSocket.

### 5. Standard host to Pico W

The source runs on the Rust std host. The show sink runs on the Pico W over the selected bounded transport.

### 6. Three manifestations

One source on the std host feeds three independently planned sinks:

```text
stdout on the std host
DOM on one browser host
LED on the Pico W host
```

All three sinks produce the same sixteen semantic receipts in the same order.

This is the foundation proof.

## Suggested code boundaries

Exact crate names may evolve, but responsibilities should remain visibly separate.

```text
conduit-semantic-core
    no_std-compatible IDs, kinds, Signal, envelopes, receipts, limits

conduit-form
    parser and validated semantic form model

conduit-plan
    host advertisements, placement validation, exact plans, diagnostics

conduit-runtime-core
    bounded source/sink lifecycle, cord semantics, activation state machine

conduit-host-std
    portable Rust std host, stdout, timers, operator/planner fixture

conduit-host-browser
    browser host instances, DOM manifestation, browser-memory and WebSocket links

conduit-host-pico-w
    no_std Pico W host, onboard LED, timers, network link

conduit-demo-signal
    portable forms, shared test vectors, receipt comparison
```

Existing crates may be reused where their current boundaries fit. Contributors should not preserve obsolete boundaries solely to avoid moving code in an unreleased reboot.

## Rules for contributors and coding agents

When implementing or reviewing this architecture, preserve these rules.

### Meaning stays above machinery

Do not add stdout, DOM, GPIO, browser, Pico, Linux, TCP, UDP, or WebSocket names to the portable form when they are realization details.

Add platform-specific behavior as an implementation and capability advertisement.

### Hosts offer, planners choose

A host must not decide global placement merely because it owns an implementation.

A planner must not assume an implementation exists merely because a kind exists.

### Plans are exact

Do not leave runtime placement, implementation selection, queue sizing, or transport choice to ambient discovery after activation begins.

### Activation is separate from planning

Creating or validating a plan must not produce effects.

### Bounds are explicit

Reject an unsupported bound rather than silently allocating more memory, truncating data, or creating a hidden queue.

### Evidence beats appearance

Tests should compare semantic receipts and deterministic state. Visual browser output and LED behavior remain useful smoke tests, not the only proof.

### Browser hosts are real hosts

Do not model one browser tab as one indivisible host. Multiple host instances in one page are a first-class requirement.

### The generic hosted implementation is Rust std

Do not call the generic hosted runtime the Linux host. Keep the core implementation portable and add operating-system-specific capabilities only when necessary.

### Do not revive the old browser test burden

No Playwright matrix, timing retries, screenshot semantics, or blocking multi-browser suite is required for this foundation.

### Fail honestly

Unavailable resources, stale boot IDs, disconnected links, malformed frames, and partial delivery must remain visible.

Do not paper over failure with retries or fallback behavior unless the plan explicitly includes that behavior.

## Deliberate non-goals

This foundation does not require:

- automatic placement optimization;
- generalized distributed consensus;
- permanent central coordination;
- transparent reconnect and replay;
- durable body identity;
- cryptographic host admission;
- `.soul` recovery;
- DHCP or DNS on the Pico W;
- HTTP serving;
- robot motor control;
- arbitrary text manifestation on an LED;
- unbounded streaming;
- real-time guarantees;
- every old Conduit feature;
- or compatibility with unreleased grammar and architecture experiments.

These are not rejected forever. They are excluded so the first proof remains small enough to build and strong enough to matter.

## Future durable body and soul layer

The longer architecture adds durable identity above the realm and host model.

A body is a long-lived distributed identity that can reorganize work around the capabilities currently offered by its admitted hosts.

A soul is the durable, verifiable continuity of that body. A `.soul` archive can serialize enough identity, history, policy, and approved state to inspect or recover the same body.

That later model should preserve the foundation established here:

```text
hosts advertise capabilities
forms describe semantic work
plans map forms to exact host realizations
activation begins only after preparation
receipts account for semantic effects
```

Durable admission will replace development-oriented realm registration. Soul history will preserve body continuity across host restarts and plan changes. Neither requires changing the meaning of `Source<T>`, `Sink<T>`, semantic kinds, capability offers, exact plans, bounded cords, or receipts.

The portable signal demonstration is therefore not a disposable toy. It is the narrow waist upon which later bodies, robotics, recovery, and richer semantic libraries can be built.

## Architectural invariants

The following statements summarize the architecture and should remain true:

```text
A form describes meaning, not placement.
A host is software, not merely a machine.
A host advertises current capabilities under explicit limits.
A capability advertisement is not authority and does not start work.
A planner combines forms, offers, links, placement policy, and bounds.
A plan makes every execution choice exact.
A plan is immutable and inactive until activation.
A cord carries typed values independently of its selected transport.
Every queue and frame is bounded.
A semantic sink may manifest differently while preserving the same value.
Receipts prove what happened.
The same form runs across browser, Pico W, and Rust std hosts.
```

## Decision record

This document captures the following current decisions:

1. Begin with three host profiles: browser, Pico W, and portable Rust std.
2. Treat multiple browser host instances as real independent hosts.
3. Use `Source<T>` and `Sink<T>` as the two fundamental structural cell shapes.
4. Use a finite alternating `Signal` as the first portable value.
5. Use one semantic show sink with three host-specific manifestations.
6. Keep forms free of platform and transport details.
7. Let hosts advertise exact implementations and limits.
8. Let the planner produce an immutable exact plan.
9. Separate planning from activation.
10. Require bounded cords, frames, queues, and executions.
11. Use machine-readable receipts as the cross-host correctness proof.
12. Defer durable body and soul mechanics until the portable execution waist works.

Issue [#347](../../issues/347) is the implementation vehicle for this architecture. Changes that contradict this document should either update the document through explicit review or explain why the architecture itself has changed.