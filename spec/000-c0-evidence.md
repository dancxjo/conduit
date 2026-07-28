# C0 — Brownfield evidence baseline

Status: candidate evidence ledger  
Baseline date: 2026-07-28

## Purpose

C0 records why Conduit exists before greenfield implementation makes its own
structure appear inevitable. Each normative feature must remain traceable to a
real requirement in Tongues, Netherwick, Psyched, portability, or safety.

## Tongues

The Tongues graph prototype already demonstrates:

- typed directional ports;
- explicit adapters, mergers, splitters, and fan-out;
- semantic node kinds separate from provider/model components;
- bounded channels and producer backpressure;
- validation diagnostics and deterministic planning;
- implementation readiness and capability requirements;
- cancellation and terminal lifecycle;
- partial, revised, committed, and cancelled streaming events;
- event time distinct from observation time;
- derivation and provenance;
- durable graph, plan, run, and timeline identities;
- presentation metadata that must not alter execution;
- artifact discovery, verification, training, resume, evaluation, and export.

Tongues also exposes the seam Conduit must correct: its current `ValueType`
closure contains speech concepts, while component specifications mix semantic
kind, implementation, provider, model, readiness, capabilities, configuration,
replacement, and UI detail.

### Tongues disposition

| Tongues concept | Conduit disposition |
|---|---|
| `GraphDocument` | `.panel` source semantics plus separate presentation |
| `GraphNode` | node instance |
| `GraphEdge` | bounded cord |
| endpoint | typed port reference |
| `NodeKindSpec` | semantic node contract |
| `ComponentSpec` | decomposed implementation manifest and requirements |
| `GraphCatalog` | one possible registry of domain manifests |
| readiness | time-bound result of implementation/host/artifact/grant resolution |
| compiled graph | exact execution plan |
| execution record | immutable run evidence |
| starter graph | ordinary exported composite-node example |

Speech types and nodes remain in Tongues. Conduit must be able to carry and
execute them without defining them.

## Netherwick

Netherwick demonstrates requirements that a speech-only extraction could miss:

- one semantic bodily capacity implemented on Linux and Pico W differently;
- a body-local controller retaining final physical authority;
- bounded commands with TTL, heartbeat, lease, and safety preemption;
- ordered physical events and missing-history detection;
- explicit possession and revocation;
- transport loss that cannot degrade into continued best-effort motion;
- immutable experience bundles and durable jobs;
- candidate artifact validation, activation, and rollback;
- host discovery, identity, proof, pairing, and capability reports;
- constrained firmware that must reject incompatible work before partial start.

Robot types and safety policies remain in Netherwick domain contracts. Conduit
provides identity, ports, bounded flow, lifecycle, authority, host observation,
planning, and evidence.

## Psyched

Psyched demonstrates the boundary on the other side:

- host bootstrap and provisioning;
- ROS, container, service, and systemd orchestration;
- module installation;
- cockpit asset deployment;
- physical host topology.

These are useful consumers of Conduit capability reports and deployment
artifacts, but Conduit does not become a provisioning system. It conducts live
computations on available hosts; it does not converge infrastructure toward a
desired state.

## Portable-core constraint

Linux and an RP2040-class device share semantic contracts, not identical
capabilities.

The portable execution subset must admit:

- `#![no_std]`;
- no required global allocator;
- exact prevalidated plans;
- fixed maximum node, port, cord, queue, and value counts;
- pre-accounted bounded storage;
- compact numeric IDs mapped to stable plan identities;
- host-supplied implementations and bindings;
- deterministic rejection before partial execution.

It need not parse `.panel`, discover implementations, provision hosts, load
Python, or execute every domain node.

## Preserved decisions

- Runtime objects are node, typed port, and cord.
- A panel document may define exported composite nodes; `Panel` is not a
  separate runtime kind.
- Internal node ports remain transparently patchable through explicit exports.
- Nodes do not universally have stdin, stdout, or stderr.
- Hosts advertise; implementations require; the resolver matches and explains.
- Every live cord is bounded.
- Pressure, cancellation, delivery, ordering, and terminal behavior are
  semantic.
- `.panel` is editable source.
- Compilation is validation, resolution, and lowering, not necessarily bytecode
  or native code generation.
- `conduct [--check | --explain | --run] [PANEL | -]` is canonical; run is the
  default.

## Product boundary

These do not become Conduit primitives merely because products expose them:

- browser navigation, dashboards, forms, and catalog pages;
- shell completion and arbitrary CLI commands;
- model-marketplace policy;
- mutable transcript correction;
- node positions, faceplates, notes, frames, and visual cable paths;
- host provisioning;
- domain-specific safety or linguistic meaning.

## Exit criterion

C0 is sufficient when every proposed core feature cites one of these cases or
an explicit greenfield safety/portability constraint, and when ordinary product
behavior is explicitly kept outside the kernel.
