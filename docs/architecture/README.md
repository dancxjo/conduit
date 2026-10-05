# Architecture reference

Start with [hosts and execution](hosts.md) for the working model, or the
[handbook tour](../../wiki/Architecture-tour.md) for a first introduction.
The [canon](../conduit-canon.md) owns enduring intent; [STATUS](../../STATUS.md)
separates development implementation from its proof limits. A design contract
is not itself evidence that every target implements it.

## Execution, planning, and identity

- [Hosts and execution](hosts.md)
- [Callable compatibility and semantic realization](functional-compatibility.md)
- [Claims and deterministic resolution](claims.md)
- [Identity classes and stability](identity-classes.md)
- [Plan-to-kernel lowering](plan-kernel-lowering.md)
- [Portable planner](portable-planner-capability.md)
- [Continuous execution](continuous-execution.md)
- [Explicit state and delay](explicit-state-delay.md)
- [Finite lifetime and capacity audit](finite-lifetime-capacity-audit.md)
- [Finite computation and universal extension](decidable-default-universal-extension.md)
- [Deadline and WCET regions](deadline-wcet-regions.md)
- [Compute resources](../r2-compute-resources.md) and [timing profiles](../timing-profile.md)

## Bodies, resources, and lifecycle

- [Body lifecycle boundaries](body-lifecycle-waists.md)
- [Durable continuity](durable-continuity.md)
- [Optional pre-play hold](pre-play-hold.md)
- [Bounded addressable resources](resources.md)
- [Protected resource bindings](protected-resource-bindings.md)
- [Bounded copy task](bounded-copy-task.md)
- [Emergency authority reduction](emergency-control.md)

## Authority, effects, and confinement

- [Base capabilities](base-capabilities.md)
- [Device protocols in plots and migration audit](device-protocol-plots.md)
- [Consequential effects](consequential-effects.md)
- [Implementation confinement](implementation-confinement.md)
- [Confined gear profile](confined-gear-profile.md)
- [Hosted base confinement](hosted-base-confinement.md)
- [Isolated HTTP base](isolated-http-base.md)
- [ConduitOS protection domains](conduitos-protection-domains.md)
- [Federation security](federation-security.md)
- [Adversarial acceptance](../security/adversarial-acceptance.md)

## Lines, interoperability, and observation

- [Plan-sealed lines](route-candidates.md)
- [Logical sessions and attachments](session-route-attachment.md)
- [Bounded session resume](route-machine-session-resume.md)
- [Protected line sessions](protected-line-session.md)
- [Connection-envelope wire format](connection-envelope-wire.md)
- [Interop membrane](interop-membrane.md)
- [ROS 2 base](ros2-base.md)
- [Observation, planning, and execution control loop](topology-planning-play-control-loop.md)
- [Host observatory](host-observatory.md)
- [Structured diagnostics](structured-diagnostics.md)
- [Machine-readable proof classes](proof-classes.md)

## Hosts and human presentation

- [Host composition](../host-architecture.md)
- [Face, mask, and show](../presenter-hourglass.md)
- [Portable input semantics](../input-semantics.md)
- [Universal face grammar](presentation-grammar-conformance.md)
- [Portable presentation renderer](presentation-renderer.md)
- [Application theme in Patchbay](patchbay-theme.md)
- [Browser make inventory](browser-make-inventory.md)
- [Browser identity and membership](browser-host-membership.md)
- [Native Patchbay file base](native-file-base.md)
- [Optional network attachment](network-capability.md)

## Semantic ownership and domain work

- [Portable catalog and std offers](semantic-catalog.md)
- [Semantic type ownership](semantic-type-ownership.md)
- [Form ownership audit](form-ownership.md)
- [Plot and Form language migration audit](plot-form-language-migration.md)
- [Portable navigation](portable-navigation.md)
- [Pete Brainstem migration](pete-brainstem-migration.md): retained provenance and remaining physical-proof gates
- [Embodied experience](embodied-experience.md): architectural direction, not a completed capability list

## Historical checkpoints

The [history index](../history/README.md) retains earlier host/kernel designs,
Patchbay milestones, physical diagnostics, and CI records. Their APIs, commands,
and stop lines describe their named checkpoint. Use current references above
for new work; historical evidence does not change merely because code evolves.
