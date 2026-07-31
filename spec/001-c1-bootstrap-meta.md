# C1 — Bootstrap meta-model

Status: candidate  
Version: 0.1.0

## Scope

C1 specifies how Conduit specifications, descriptors, conformance claims, and
fixtures are structured. It does not fix the final `.panel` grammar, plan byte
encoding, Rust API, Patchbay layout format, or universal package format.

## Specification layers

| Layer | Owns |
|---|---|
| Meta | descriptor rules, identity, versioning, compatibility, hashing, conformance |
| Semantic | types, node contracts, ports, lifecycle, effects, authorities |
| Implementation | executors, artifacts, targets, resource requirements |
| Host | observed capabilities/resources, bindings, grants, freshness |
| Authoring | `.panel` grammar, composition, source constraints |
| Execution | exact plans, flow, lifecycle, cancellation, evidence |
| Presentation | layout, faceplates, notes, visual routing, dashboards |

Presentation may refer to source and run identities. It does not feed execution
semantics.

## Descriptor kinds

Later encodings lower to an encoding-independent abstract value model and
represent:

- `TypeContract`;
- `PortContract`;
- `NodeContract`;
- `ImplementationManifest`;
- `ArtifactManifest`;
- `CapabilityReport`;
- `PanelDocument`;
- `ExecutionPlan`;
- `ExecutionEvent`;
- `Diagnostic`;
- `DomainProfile`;
- `ConformanceManifest`;
- `PresentationDocument`.

Source, manifest, host observation, plan, run evidence, and presentation have
independent identities and versions.

## Identity

Semantic contract identities use:

```text
namespace/local.name
```

Examples:

```text
std/text
tongues/audio.stream
netherwick/can-host-wifi
```

Identity does not include a provider, host, implementation, artifact location,
or display label. Source may accept version ranges. Resolved plans pin exact
versions and semantic hashes.

Local node, port, cord, export, and binding IDs form deterministic instance
paths when composite nodes expand.

## Versions and compatibility

Meta-specification, source grammar, semantic contract, implementation manifest,
plan encoding, and evidence encoding versions are independent.

Compatibility is directional. A claim states which consumer accepts which
producer, which implementation satisfies which contract, or which runtime
executes which plan.

The current form query roles, three-valued outcomes, backward/forward definitions,
substitution variance, and exact migration identity are normative in
[`004-directional-compatibility.md`](004-directional-compatibility.md).

Breaking changes include:

- changing units, clock, coordinate frame, delivery, or terminal meaning;
- removing or renaming a port;
- changing port direction or type incompatibly;
- tightening a required input;
- adding authority or effects;
- weakening cancellation or determinism;
- reusing a field, variant, port, or tag for new meaning.

Adapters are explicit contracts and plan nodes. Shape similarity is not semantic
type equivalence.

## Semantic hashes

Semantic hashes exclude prose, labels, source spans, comments, layout, editor
state, build timestamps, and non-critical annotations.

They include every fact affecting validation, resolution, authority, resource
use, or execution.

The baseline digest is algorithm-qualified SHA-256. Canonical descriptor form
current form and its current cross-language vectors are defined by
[`003-canonical-descriptor.md`](003-canonical-descriptor.md). A plan
records every exact semantic hash that influenced it.

## Contracts

A type contract defines semantic shape, constraints, sensitivity, retention,
and traits. Variable-sized values require a maximum before bounded or embedded
execution.

A port contract defines:

- stable ID;
- input or output direction;
- semantic value type;
- required/optional presence;
- connection cardinality;
- delivery shape;
- relevant temporal and terminal semantics.

A node contract defines:

- configuration type;
- ports;
- lifecycle;
- state/checkpoint behavior;
- determinism inputs;
- effects and requested authorities;
- typed failures;
- semantic capabilities;
- portability constraints.

An implementation manifest separately declares its executor, entrypoint,
artifacts, host/resource/authority requirements, profiles, and reproducibility.
“Ready” is a resolution result for a particular observation, not a timeless
manifest property.

## Panel semantics

The semantic panel document contains imports, exported composite definitions, a
root definition, instances, cords, explicit port exports, selection constraints,
authority requests, host constraints, and an optional presentation reference.

A composite node is patched like a primitive node. Expansion must reject
definition cycles, incompatible exports, unresolved references, type mismatch,
illegal cardinality, and impossible flow constraints.

The grammar currently implemented in `conduit-panel` is a strict executable
seed. It is not yet the complete normative authoring grammar.

## Flow

Source constrains flow; a resolved plan selects exact finite policy.

At minimum, flow distinguishes:

- finite capacity;
- stream, state, batch, artifact, and control delivery;
- block, reject, coalesce, sample, declared-disposable drop, disconnect, and
  failure pressure behavior;
- coupled and isolated fan-out;
- ordering and merge policy;
- remote acknowledgement, retry, and disconnect behavior;
- cancellation and terminal propagation.

Dropping or coalescing is illegal unless the carried type declares the required
disposable or replacement semantics.

## Plans and evidence

An execution plan pins:

- source hash;
- resolver identity/version/policy;
- semantic contracts;
- implementations;
- host reports and freshness;
- artifacts;
- grants;
- resource budgets;
- nodes, ports, cords, bindings, and exports;
- exact flow and lifecycle policy.

A constrained executor verifies the plan and its limits before starting any
node. Specification 011 freezes the exact schema, canonical identity,
freshness rules, resource accounting, and allocator-free validation profile.

Execution events identify the run and exact plan and preserve ordered sequence,
event/observation time where applicable, subject, typed payload, derivation, and
terminal state. Mutable correction or analysis refers to immutable evidence; it
does not rewrite it.

## Diagnostics

Stable families are reserved:

```text
CND-SRC-*  source and parsing
CND-ID-*   identity and reference
CND-TYP-*  semantic type
CND-PRT-*  port and cardinality
CND-CMP-*  composition and export
CND-FLW-*  flow and boundedness
CND-IMP-*  implementation satisfaction
CND-HST-*  host capability/resource
CND-AUT-*  authority and sensitivity
CND-ART-*  artifacts
CND-PLN-*  planning and plan integrity
CND-RUN-*  execution and lifecycle
CND-EVD-*  evidence integrity
CND-EXT-*  extensions
```

Static diagnostics, typed error values, node failure, run failure, and process
failure are distinct.

## Portability profiles

`core-no-alloc` requires exact plans, fixed maxima, pre-accounted queues, no
dynamic discovery or source parsing, compact IDs, and deterministic pre-start
rejection.

`core-alloc` permits allocation but not unbounded live flow.

`hosted-local` adds dynamic discovery and local process/device/file bindings
under explicit authority.

`hosted-distributed` adds remote identity, carrier, ordering, acknowledgement,
retry, disconnect, freshness, and cancellation semantics.

## Conformance classes

Claims name exact versions and one or more of:

- descriptor reader;
- semantic validator;
- panel lowerer;
- resolver;
- plan encoder;
- executor;
- host reporter;
- evidence emitter;
- lossless editor;
- domain provider;
- implementation provider.

“Conduit compatible” without class, profile, versions, and suite result is
incomplete.

## Candidate requirements

| ID | Obligation |
|---|---|
| META-001 | Preserve specification-layer ownership |
| META-002 | Give every semantic/runtime object stable kind-appropriate identity |
| META-003 | Version independent layers independently |
| META-004 | Pin meaning with encoding-independent semantic hashes |
| META-005 | Admit domain-owned semantics without core variants |
| META-006 | Separate semantic contracts from implementations |
| META-007 | Support exported recursive composite nodes |
| META-008 | Resolve every live cord to bounded compatible flow |
| META-009 | Define and evidence cancellation and terminal lifecycle |
| META-010 | Resolve deterministically from recorded inputs |
| META-011 | Produce exact self-verifiable execution plans |
| META-012 | Resolve scoped authority before effects |
| META-013 | Verify artifact integrity and provenance |
| META-014 | Keep execution evidence immutable and attributable |
| META-015 | Keep presentation non-semantic |
| META-016 | Separate compilation from code generation and packaging |
| META-017 | Admit allocator-free portable execution |
| META-018 | Produce stable attributable diagnostics |
| META-019 | Require evidence and fixtures for new normative features |

## C1 exit

Canonical descriptor form current form and directional compatibility algebra
current form are stable with current fixtures. The remaining meta-model sections
stay candidate until their owning semantic, planning, evidence, and packaging
specifications stabilize.
