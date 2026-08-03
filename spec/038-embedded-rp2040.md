# Allocator-free embedded execution profile current form

Status: candidate normative C5 contract

Embedded-profile schema: 1

Compact static-plan schema: 1

RP2040 HIL protocol: 1

## Boundary

This specification binds a constrained firmware executor to an already
validated exact `ExecutionPlan`. The device receives a compact static
representation carrying the full plan hash and one identity-bound embedded
profile. It does not parse `.panel` source, resolve a registry, discover a
host, fetch or load an artifact, acquire authority, provision networking, or
update firmware.

Host-operation ordinals are build-time driver bindings only. Each ordinal is
resolved to one exact ordinary plan authority and resource and generated with
its semantic action, selected resource and host, effect, grant, lease, and
commit-profile identities, and use-time-check requirement. An unbound ordinal
terminates before the host adapter is called. Firmware adapters therefore
dispatch on generated semantic actions rather than maintaining a private
numeric operation registry.

The full `ExecutionPlan` remains the semantic and execution identity. The
compact representation is generated firmware input, not a second plan type.
Firmware build identity, fresh capability report, boot/run identity, execution
evidence, and HIL presentation remain distinct.

`conduit-embedded` is `#![no_std]`, has no allocator dependency, and exposes
only caller-owned fixed storage. It depends on `conduit-core`; RP2040 startup,
USB, clocks, and peripheral bindings live in the separate
`conduit-rp2040-hil` firmware package.

## Identity-bound profile and preflight

`conduit/embedded-execution-profile` current schema pins finite maxima for:

- nodes, cords, aggregate ports, host operations, and nesting;
- queue slots and bytes in one fixed inline value representation;
- normative evidence records;
- timers and wake interests per node;
- unambiguous wrapping timer delay;
- complete static-RAM, reviewed stack, and flash budgets.

Every selected value must be nonzero and no larger than the exported
implementation ceiling. The profile hash changes with every bound.

The static representation contains semantic node paths, selected
implementation IDs, exact host-operation bindings, port counts, step-work
limits, nesting, exact cord endpoints, non-overlapping queue slot ranges,
capacity, and value width.
Preflight checks the schema, full plan/profile pins, all counts and checked
totals, caller storage shape, endpoint/port validity, queue overlap, and the
supported feature subset before `prepare` is called (`EMB-001` through
`EMB-005`).

current form supports one cord per local input/output ordinal. Fan-out, merge,
dynamic pools, checkpoints, distributed cords, and stateful hot replacement
therefore fail during preflight or host resolution. This is an honest subset,
not an implicit rewrite. The shipped generic RP2040 firmware initializes
clocks and USB CDC only. Its fresh report advertises no Wi-Fi, AP, CYW43, or
Zenoh-Pico capability. A future Pico W report requires a distinct exact
firmware artifact that links and initializes CYW43 plus its bounded network
services before it may advertise those capabilities.

## Fixed execution

`EmbeddedStorage<N,C,P,Q,V,E,T,I>` owns:

- one array of inline queue values and per-cord ring indices;
- node terminal/ready state and exact wake-interest sets;
- a fixed timer table;
- fixed normative evidence; and
- port scratch.

No executor operation grows any collection. The plan-visible profile can
select limits below the compiled storage shape; the selected evidence, timer,
interest, value, and graph bounds are still enforced.

An application supplies one concrete driver enum implementing
`EmbeddedNode`; this permits heterogeneous generated bindings without trait
objects or dynamic loading. A step receives only fixed input snapshots,
terminal facts, output-capacity facts, current wrapping tick, exact work
ceiling, and a host-operation broker. It explicitly consumes inputs and stages
outputs. Progress/completion commits all staged work atomically. Pending,
yield, failure, invalid interest, or invalid work rolls back it.

Pending nodes wake only for named input, output, timer, or cancellation
interests. Ready nodes run in deterministic round-robin order. A timer delay
must be within one unambiguous wrapping half-range and the selected profile
ceiling. Decision and evidence exhaustion are terminal; evidence never spills
into logging.

Abort cancellation invokes every driver cancellation hook, clears every
queued value, and emits attributed cancellation/terminal evidence. Natural
success requires all nodes terminal and all queues empty. Every event carries
the immutable full plan hash and separate boot/run identity.

## Host services and capability report

`EmbeddedHostServices` is a bounded opaque request/reply boundary. Operation
bindings are generated from exact plan resources; request and reply bytes use
the fixed value representation. Peripheral FIFOs, DMA, radio firmware queues,
USB buffers, interrupt state, and Embassy/RTOS storage remain host/backend
charges and must appear in the execution profile and capability report.

The reference fixture emits a normal capability-report current schema with:

- `Firmware` executor, `thumbv6m-none-eabi` target, and exact static-step ABI;
- firmware reporter/trust and current firmware/profile constraints;
- exact memory, timer, transport, and evidence pools; and
- a separately described Pico W Wi-Fi capability.

It does not invent membership, authority, durability, Zenoh, TLS, provisioning,
or hot replacement.

## Replacement boundary

Cold replacement is supported outside an active run. Quiescent replacement is
accepted only when the old and new generation storage fit an explicit overlap
budget. Stateful hot replacement is rejected. These checks do not create the
plan-transition contract owned by issue #57 and never mutate an active plan.

## Firmware artifact and HIL protocol

`conduit-rp2040-hil` links a real Cortex-M0+ ELF with RP2040 boot/runtime,
clocks, ring-oscillator boot identity, USB CDC, one statically placed executor
storage block, the representative sensor → threshold → indicator bindings,
and the complete allocator-free executor.

The linked binary and `firmware_contract` host oracle share that exact static
topology, embedded profile, and driver path. The oracle round-trips the same
header/event codecs and requires the physical runner's values, pressure
transitions, attribution, and terminal event, preventing a capacity change in
the firmware from silently invalidating the HIL expectation.

`cargo xtask embedded-gate` cross-links this ELF, reads its load-image and
data/BSS sizes, rejects allocator symbols, and reports:

- flash as linked ELF text plus data;
- static RAM as linked ELF data plus BSS; and
- stack as the explicit reviewed profile ceiling, not a fabricated ELF
  measurement.

HIL protocol 1 uses fixed big-endian frames with distinct request, run-header,
and event magic. The host supplies a nonce, expected full plan hash, and
decision ceiling. The run header repeats the nonce and plan together with a
SHA-256 identity over the exact Cargo lockfile, embedded executor source,
core source, firmware source/manifest, memory layout, Rust compiler, target,
and Cargo profile used for the build. It also carries the identity of a fresh
generic capability report produced for that HIL run from the exact
firmware/profile, target, ABI, fixed pools, and honest Wi-Fi capability. Every
event repeats the nonce, plan, random boot identity, run sequence, and exact
evidence sequence. The HIL runner recomputes the expected release-firmware
identity from its checkout and rejects a foreign build, absent capability
report, version, plan/session mismatch, gap, failure status, missing
lifecycle/value/pressure/terminal evidence, or values differing from the
desktop oracle.

An HIL fixture is not recorded as passed merely because the firmware links or
the simulator passes. `cargo xtask rp2040-hil --require-hardware` succeeds only
after a unique USB-CDC device completes the physical exchange. Flashing and
device enrollment remain explicit operator actions outside the resolver.

## Stable diagnostics

- `CND-EMB-001` invalid profile or profile identity mismatch
- `CND-EMB-002` invalid compact static-plan structure
- `CND-EMB-003` selected profile/storage exceeded or feature unsupported
- `CND-EMB-004` checked accounting overflow
- `CND-EMB-005` fixed value or port/queue access violation
- `CND-EMB-006` invalid or overflowing exact wake interests
- `CND-EMB-007` ambiguous/invalid timer or timer storage exhausted
- `CND-EMB-008` step work exceeded or false progress/yield
- `CND-EMB-009` prepare/start failure before a valid run
- `CND-EMB-010` invalid run, decision/evidence exhaustion, or irrecoverable stall
- `CND-EMB-011` node-reported failure
- `CND-EMB-012` unsupported replacement level or overlap
- `CND-EMB-013` unsupported or malformed HIL protocol
- `CND-EMB-014` runtime driver identity differs from the generated binding
- `CND-EMB-015` driver requested a host operation absent from its generated binding

## Conformance and evidence status

`conformance/c5/embedded-rp2040.json` contains independently executed
preflight, executor, pressure, timer-wrap, cancellation, reboot, HIL-codec,
and replacement cases. These include the maximum supported graph, duplicate
semantic-mapping rejection, retained step-operation faults, and evidence
reservation before host effects. `conformance/c5/rp2040-budgets.json` owns
the linked artifact budgets.

The simulator and linked artifact establish software conformance. Closure of
issue #28 additionally requires a retained physical HIL report from the same
commit; an absent device is an explicit unexecuted condition, not a passing
fixture.

## Normative requirements

| ID | Obligation |
|---|---|
| EMB-001 | Keep source parsing, resolution, allocation, loading, provisioning, and domain semantics out of firmware execution |
| EMB-002 | Bind one compact static representation to the immutable full plan hash and exact embedded-profile hash, and every host call to one exact plan authority and resource |
| EMB-003 | Make node, cord, port, host-operation, queue, value, evidence, timer, interest, nesting, RAM, stack, and flash ceilings explicit |
| EMB-004 | Reject unsupported counts, topology, features, and caller storage before prepare or start |
| EMB-005 | Use only caller-owned fixed storage and retain no allocator linkage |
| EMB-006 | Preserve bounded step, atomic transaction, exact wake, cancellation, and terminal semantics |
| EMB-007 | Handle timer wraparound within one explicit unambiguous half-range |
| EMB-008 | Attribute every normalized lifecycle, value, pressure, cancellation, and terminal event to exact plan and run identities |
| EMB-009 | Describe the actual firmware build, ABI, pools, target, radio/peripherals, and supported versions in a fresh generic capability report |
| EMB-010 | Reject unimplemented Zenoh/distributed/security/replacement features during resolution or preflight |
| EMB-011 | Link and measure a real RP2040 ELF without calling an archive a flash/RAM result |
| EMB-012 | Keep physical HIL proof distinct from simulator, cross-compile, and codec proof |
