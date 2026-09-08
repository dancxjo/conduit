# Architecture guide

Start with [Hosts and execution](hosts.md) for the working model, then the
[canon](../conduit-canon.md) for durable intent and invariants. You do not need
to read this entire directory to contribute. Choose the reference for the
boundary you are changing; the [contribution guide](../../CONTRIBUTING.md)
explains how to get started.

[Current status](../../STATUS.md) separates implemented behavior from its proof
limits. The [roadmap](../roadmap.md) links active work. To see the native system
in use, visit the [ConduitOS visual evidence](../visual-evidence.md).

These documents have different jobs: references explain contracts, design notes
preserve decisions, and milestone records preserve evidence for a particular
slice. A historical stop line or command is not a new contributor requirement.
Source and focused conformance tests are the reference for exact APIs; old
proposals do not override the current canon.

## Execution and planning

- [Hosts and execution](hosts.md)
- [Functional compatibility: the face is the contract](functional-compatibility.md)
- [Plan-to-kernel lowering](plan-kernel-lowering.md)
- [Portable planner capability](portable-planner-capability.md)
- [Continuous execution over finite Plays](continuous-execution.md)
- [Explicit state/delay contract](explicit-state-delay.md)
- [General-purpose computation under explicit finite bounds](decidable-default-universal-extension.md)
- [Deadline and WCET regions](deadline-wcet-regions.md)
- [Implementation confinement and admitted authority](implementation-confinement.md)
- [Portable catalog and hosted std offer boundary](semantic-catalog.md)

## Body, resources, and effects

- [Body lifecycle architectural waists](body-lifecycle-waists.md)
- [Optional pre-Play HOLD](pre-play-hold.md)
- [Durable system continuity](durable-continuity.md)
- [Bounded addressable Resources](resources.md)
- [Protected resource bindings](protected-resource-bindings.md)
- [Bounded Copy-a-file execution](bounded-copy-task.md)

## Lines and observation

- [Plan-sealed Lines](route-candidates.md)
- [Logical sessions and Line attachments](session-route-attachment.md)
- [Deterministic Lines and bounded session resume](route-machine-session-resume.md)
- [Connection-envelope wire format](connection-envelope-wire.md)
- [Observation, planning, realization, and execution control loop](topology-planning-play-control-loop.md)
- [Host Observatory](host-observatory.md)
- [Structured diagnostics v1](structured-diagnostics.md)
- [Machine-readable proof classes](proof-classes.md)

## Browser and presentation

- [Browser Host fabrication inventory](browser-fabrication-inventory.md)
- [Browser Host identity and Body membership](browser-host-membership.md)
- [Portable presentation renderer contract](presentation-renderer.md)
- [Patchbay renderer theme contract](patchbay-theme.md)
- [Native Patchbay protected file base](native-file-base.md)
- [Optional network attachment capability](network-capability.md)

## Domain migration and hardware records

These retain provenance and physical proof boundaries. They do not establish
the state of hardware attached today.

- [Pete Brainstem migration ledger](pete-brainstem-migration.md)
- [Pete R23 carrier audit](pete-r23-carrier-audit.md)

## Historical milestones and design proposals

Preserved at their existing URLs for issue and evidence links. These describe
their named checkpoint, including superseded APIs, syntax, and implementation
limits. Use the references above for new work.

- [Portable Host Architecture](portable-hosts.md)
- [Conduit Host Specification](host-specification.md)
- [Reboot Kernel M0 Sign](reboot-kernel-m0.md)
- [Salvage S1 kernel](salvage-kernel-s1.md)
- [Salvage S2 exact planning](salvage-planning-s2.md)
- [Kernel takeover integration gate](kernel-takeover.md)
- [Actual browser kernel host checkpoint](browser-host-s4.md)
- [Native Patchbay shell](native-patchbay-shell.md)
- [Native Patchbay topology projection](native-patchbay-topology.md)
- [Native Patchbay canonical Form editor](native-patchbay-form-editor.md)
- [Native Patchbay Plan and Play control](native-patchbay-control.md)
- [Native Patchbay primitive GUI](native-patchbay-gui.md)
