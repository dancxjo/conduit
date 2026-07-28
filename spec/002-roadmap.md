# Plan C implementation roadmap

## Current foundation

The repository currently proves:

- allocator-free semantic identifiers, node/port contracts, bounded cords, and
  structural plan validation;
- an editable flat `.panel` seed grammar;
- hosted built-in implementation registration;
- deterministic endpoint and implementation resolution;
- type, cardinality, capacity, and cycle validation;
- the exact bounded FlowPolicy algebra and allocator-free reference queue;
- exact explanation output;
- finite acyclic execution;
- process stdin/stdout/stderr as explicit node implementations;
- the canonical `conduct` modes, with run as default;
- hosted tests and an RP2040-target core build.

It does **not** yet claim:

- live streaming scheduling;
- full pressure-policy execution;
- exported composite-node source syntax;
- implementation manifests and host capability reports;
- authority grants;
- artifact resolution;
- canonical plan bytes;
- durable event evidence;
- remote cords;
- Patchbay UI.

## C2 — Semantic core

Freeze and test:

1. type references and semantic traits;
2. port delivery, cardinality, time, replacement, and terminal contracts;
3. node lifecycle and cancellation state machines;
4. cord flow-policy algebra;
5. composite-node expansion and port exports;
6. authority request/grant vocabulary;
7. resolved execution-plan semantics;
8. execution-event semantics;
9. portable limits and scratch-memory contracts;
10. canonical positive and negative fixtures.

Exit: an independent reference model and `conduit-core` agree on every fixture.

## C3 — Authoring and compilation

Complete `.panel` syntax, imports, exported definitions, configuration typing,
selection constraints, source spans, diagnostics, and deterministic lowering.

Exit: `conduct --check` and `--explain` produce frozen results for all fixtures.

## C4 — Live executor

Implement bounded queues, fan-out isolation/coupling, pressure behavior,
ordering, cancellation, terminal propagation, checkpoints, and evidence.

Exit: stress fixtures demonstrate bounded memory and correct slow-consumer
behavior.

## C5 — Host resolution

Implement implementation and artifact manifests, fresh capability/resource
reports, bindings, scoped grants, deterministic selection, and exact plans.

Exit: one semantic capacity resolves to different Linux and Pico W
implementations with complete explanations.

## C6 — Tongues profile

Move domain types, node contracts, components, starters, stream events, and
relevant fixtures out of the Tongues pipeline prototype without breaking its
product surfaces.

Exit: the five current speech arrangements conduct through Conduit and Tongues
owns every speech-specific contract.

## C7 — Netherwick profile

Describe existing body/sensor/control capacities first, then wrap current
firmware and hosted behavior without weakening authority, TTL, stop, identity,
or ordered evidence.

Exit: current physical behavior and hardware-in-loop stop evidence remain
equivalent through Conduit plans.

## C8 — Patchbay

Build the visual authoring and evidence environment over stable source
identities, diagnostics, plans, and events.

Exit: presentation changes never change plan identity, and exported composite
nodes remain transparently patchable.
