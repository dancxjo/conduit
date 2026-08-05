# Salvage S2 exact planning

Issue #363 replaces the reboot planner's single-value-kind capability claim
with exact semantic and execution-profile bindings. This document records the
accepted boundary of the completed S2 slices; the remaining plan facts stay
open.

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

## Separate form identities

The planner now carries three different identity types end to end:

- `SourceDocumentId` hashes the exact authored UTF-8 document, including
  comments and spelling;
- `CheckedFormId` hashes the canonical checked semantic graph; and
- `ExpandedFormId` hashes the expanded graph domain.

The current small parser has no nesting, so expansion is deliberately the
identity transform over the checked graph while retaining a distinct,
domain-separated identity. S3 must recompute `ExpandedFormId` from the actual
expanded cells and ports when nesting is restored.

Every fragment and top-level plan binds all three values. Fragment and plan
canonicalization hash all three, verification requires every fragment to agree
with the top-level plan, and hosted/Observatory reports render them separately.
A comment-only source edit therefore changes only `SourceDocumentId`; a
semantic edit changes checked and expanded identity as well.

## Startup, cancellation, terminal, and evidence contracts

Every fragment now binds an explicit startup dependency graph and a
deterministic local startup order. Each cord makes its sink a prerequisite of
its source, so downstream placements are ready before upstream placements can
emit. The planner rejects cyclic dependency graphs. A remote cord can name a
placement on another host in the dependency graph; a host checks the ordering
constraint when both endpoints are local, while later remote-link work must
prove cross-host readiness before activation.

The first executable policy profile is deliberately narrow:

- cancellation is `CancelAllAndRejectLateCompletion`; and
- terminal completion requires every planned placement and connection.

Preparation rejects any other resealed policy. It also reconstructs the exact
terminal and mandatory-evidence descriptors from placements and connections,
so deleting, reordering, or inventing descriptors cannot relax the contract.

`EvidenceStorageBudget` binds independent item and byte capacities. The
first-profile byte rule charges one discriminant byte per mandatory descriptor
plus the UTF-8 byte length of its placement or connection identity when one is
present. Planning fails if the requirement cannot be represented by the public
budget types. Preparation fails closed if either capacity is below the exact
mandatory requirement.

The hosted reboot runtime now allocates the plan's evidence item slots during
preparation. Each recorded event is a bounded numeric index into the fragment's
sealed expected-evidence table, so execution does not clone identity strings or
grow a hidden per-event allocation. Inspection reconstructs a
`MandatoryEvidenceReport` with expected and recorded descriptors, the bound
budget, serialized bytes used, the allocation shape, and an explicit overflow
flag. This mandatory log is independent of the lossy general observation ring:
terminal evidence remains complete even when that ring reports an
`EvidenceGap`. Lowering the same commitments into the S1 kernel's
`EvidenceSink` remains open integration work.

## Exact host-operation requirements

Each capability and planned operation now carries an ordered set of exact
`HostOperationRequirement` values. A requirement binds the immutable operation
contract, an optional target kind, the maximum concurrent requests, and
independent input/output byte bounds. The first executable profile uses
`conduit.host/wait@1` with no target and `conduit.host/present@1` with the exact
presentation kind. The hosted scheduler permits one outstanding action per
placement, matching the profile's bound of one.

Planning rejects empty identities, zero concurrency, duplicates, and
non-canonical ordering. Preparation compares the sealed requirements with both
the current boot-scoped capability and the installed implementation. A
post-seal mutation therefore changes fragment identity, while a resealed lie
fails against current executable truth.

Before emitting any platform effect, the hosted runtime admits the requested
contract, exact target, and encoded input size against the plan. An unplanned
request or oversized input terminates the placement without crossing the host
boundary. Presentation completion messages are independently bounded; an
oversized completion is rejected without consuming the pending request, so a
subsequent conforming completion can settle it. The semantic requirement IDs
are not yet lowered into the S1 kernel's numeric `FixedHostOperationBindings`;
that remains explicit integration work.

## Exact resource requirements and reservations

The first S2 resource profile uses three separate contracts:

- a host advertises boot-scoped `ResourceOffer` pools with an immutable pool
  identity, semantic resource class, and finite unit capacity;
- a capability and its installed implementation declare ordered
  `ResourceRequirement` class/unit pairs; and
- planning resolves each requirement to one exact `ResourceBinding` pool.

The first profile deliberately permits only one pool per required class on a
host. Ambiguity fails planning instead of hiding a provider choice. The initial
classes are timer slots and presentation slots, each used by the corresponding
wait or presentation execution profile. These are countable host-managed slots,
not claims about unmeasured RAM, browser APIs, firmware peripherals, or physical
availability.

Planning validates pool and requirement shape, resolves the pool, sums every
placement's units per host, rejects arithmetic overflow, and rejects demand
above the boot-scoped offer. The selected pool, class, and units participate in
fragment identity. Preparation reconstructs the generic requirements from the
bindings, compares them with the pinned capability and installed
implementation, verifies each pool against the current advertisement, and
recomputes aggregate use across the incoming fragment and every unreleased
plan. A successful prepare reserves those units; completion or cancellation
does not release them, while the explicit release transition does.

Resource availability remains separate from authority. An advertised pool
allows planning and reservation only; it does not grant permission to operate
on an external subject. Authority grants remain the next S2 slice. These hosted
bindings are also not yet lowered into an S1 fixed-storage resource table.

## Exact authority requirements and grants

Authority is supplied to planning independently of `HostAdvertisement`.
Capabilities and installed implementations may declare ordered
`AuthorityRequirement` values that name an authority contract, the admitted
host-operation contract, and the exact subject kind. Each requirement must
refer to a targeted host operation already declared by that execution profile;
an advertisement cannot manufacture or imply a grant.

An immutable `AuthorityGrant` additionally scopes the requirement to one exact
host, boot, and capability. Planning rejects malformed grants, a missing or
stale scope, duplicate grant identity, and more than one otherwise matching
grant. It seals the selected grant identity and complete scope as an
`AuthorityBinding` in the placement and fragment identity.

Preparation reconstructs the requirements from those bindings, compares them
with both capability and installed implementation truth, checks placement
scope, and requires exactly one current grant with the same immutable facts.
Effect admission then checks the requested host-operation subject against the
bound authority before emitting a platform effect. A deterministic adversarial
vector advertises two presentation subjects while granting only one; an
implementation request for the other subject terminates with
`AuthorityDenied` and emits no effect.

The first profile intentionally omits expiry, revocation observations,
delegation, constraints, audit envelopes, and secret material. Those facts must
be added only with corresponding runtime semantics. Existing stdout and
simulated presentation profiles remain process-owned manifestations and do not
claim external DOM, device, or physical authority. Actual adapters must declare
and receive suitable grants when they are introduced. Authority bindings are
not yet lowered into an S1 fixed-storage authority table.

## Deterministic proof

The focused vectors prove:

- checked-form identity changes when a contract revision, port identity, port
  value kind, or direction changes;
- source spelling, checked semantics, and expanded graph identity remain
  distinct, and mutating any one changes plan identity or fails verification;
- planning binds exact contract/profile identity and every port;
- planning rejects a different revision or an additional non-first port;
- planning orders sinks before sources and rejects a cyclic startup graph;
- post-seal contract, profile, and per-port mutations fail identity
  verification; and
- resealed contract, profile, and port lies fail preparation against the
  current advertisement and installed implementation; and
- post-seal and resealed startup, cancellation, terminal, and independent
  evidence-item/evidence-byte mutations fail identity or preparation; and
- the hosted mandatory-evidence allocation stays fixed from preparation through
  completion and remains complete while the general observation ring
  overflows; and
- planning, fragment identity, preparation, action admission, and completion
  admission all preserve exact host-operation contracts, targets, concurrency,
  and byte bounds; and
- planning rejects malformed, unavailable, ambiguous, overflowing, and
  exhausted resource contracts; resource binding mutations fail identity or
  preparation; and hosted reservations remain occupied until explicit release;
  and
- planning keeps grants separate from advertisements and rejects malformed,
  missing, stale, duplicate, or ambiguous authority; every binding field is
  identity-covered; preparation requires the exact current grant; and
  ungranted subjects fail before effects.

## Acceptance boundary

This satisfies the first acceptance item and the three-form-identity portion
of the second item in #363. It also binds the startup dependency,
cancellation, terminal, mandatory-evidence, and evidence-storage-budget parts
of that item, plus exact hosted host-operation, resource, and authority
requirements. S2 remains open for observed `LinkBinding` values. The planned
evidence, host-operation, resource, and authority commitments are not yet
lowered into the S1 kernel stores. The existing
`ConnectionProvider` remains a prototype until the remote-link slice replaces
it.

## Checkpoint

```text
just check-planning-s2
just check
```
