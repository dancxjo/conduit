# Salvage S2 exact planning

Issue #363 replaces the reboot planner's single-value-kind capability claim
with exact semantic and execution-profile bindings. This document records the
accepted boundary of the first S2 slice; the remaining plan facts stay open.

## Exact semantic capability slice

A `CapabilityOffer` now advertises:

- one immutable `KindContractRevision`;
- one immutable `ExecutionProfileId` implemented by the installed operation;
- the complete ordered input and output `PortDescriptor` contracts; and
- independent active-instance, queue-item, and queue-byte limits.

`CapabilityLimits.value_kind` no longer exists. Kinds own their ports in the
form catalog. Checking copies the exact contract revision and all ports into
the checked operation, and its identity includes the revision, port identity,
value kind, and direction. Planning accepts a capability only when its kind,
contract revision, and complete input/output vectors equal the checked
operation. It then binds the capability's exact execution profile into the
planned operation.

Every installed `OperationImplementation` declares the contract revision and
execution profile it realizes. Preparation compares the sealed placement with
the current boot-scoped capability offer and the installed implementation. A
resealed lie therefore fails with a distinct contract, profile, or port
failure; an unsealed post-plan mutation fails fragment identity verification.

Observatory capability and placement rows preserve and render the same exact
contract/profile facts. Hosted, browser-shaped, Pico-shaped, composite, and
standard fixtures advertise their actual per-port contracts, but retain their
existing truth classifications: this slice does not promote simulations into
adapters or physical proof.

## Deterministic proof

The focused vectors prove:

- checked-form identity changes when a contract revision, port identity, port
  value kind, or direction changes;
- planning binds exact contract/profile identity and every port;
- planning rejects a different revision or an additional non-first port;
- post-seal contract, profile, and per-port mutations fail identity
  verification; and
- resealed contract, profile, and port lies fail preparation against the
  current advertisement and installed implementation.

## Acceptance boundary

This satisfies the first acceptance item in #363. S2 remains open. Plans do not
yet bind separate source/checked/expanded identities, host-operation/resource/
authority requirements, observed `LinkBinding` values, cancellation policy,
terminal policy, or mandatory evidence storage budgets. The existing
`ConnectionProvider` remains a prototype until the remote-link slice replaces
it.

## Checkpoint

```text
just check-planning-s2
just check
```
