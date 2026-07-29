# Administrative containment v1

Status: normative seed for issue #94.

## Purpose

An exact grant proves that one effect is authorized. It does not by itself
prove that a workload cannot assemble several individually authorized effects
into an expansion of its own authority. This specification defines a
domain-neutral, allocator-free proof that separates ordinary execution from
administrative state change.

The core does not define administrator roles or administrative operation
names. A domain marks an `EffectRequirement` with an exact
`administrative_class` `PinnedDescriptor`. Absence means an ordinary effect
and preserves the v1 effect identity. Presence requires an
`AdministrativeProof` on the same exact plan authority binding.

## Distinct identities

The following identities MUST remain distinct:

1. `AdministrativeProposal.identity` identifies the requested operation,
   exact subject, all beneficiaries, requester provenance, time window, and
   optional delegation, protected handle, and ceremony.
2. Each `AdministrativeApproval.identity` identifies one exact proposal,
   policy, approver realm/entity/key/profile/source plan and epoch, declared
   failure domain, status, and time window.
3. `AdministrativeCommit.identity` identifies the proposal, policy, complete
   approval set, committer provenance, and commit tick.
4. `AdministrativeExecution.identity` identifies one execution authorization,
   exact proposal and commit, executor provenance, and validity window.
5. `AdministrativeControlRecord` is immutable evidence naming a stage
   identity; it is not any of the four authorization objects.

Transport, process, host, or cord crossings MUST NOT replace the retained
`source_plan` and `source_epoch` provenance.

## Exact subject and replay boundary

`AdministrativeSubject` binds realm, entity, plan, epoch, optional artifact
digest, and optional budget descriptor. Presence and absence are exact.
Validation at plan creation and use compares the complete subject. A proof
issued for any other realm, entity, plan, epoch, artifact, or budget fails with
`CND-CTN-005`.

Every subject benefiting from the change MUST appear in `beneficiaries`.
When the policy requires beneficiary independence, an approval originating
from a benefiting entity or plan/epoch is self-supporting. Requester
independence applies the same rule to the requesting provenance.

## Policy and thresholds

`ContainmentPolicy` names:

- the exact domain-owned effect class;
- every allowed realm/entity/key/profile tuple;
- the exact committer and executor realm/entity/key/profile tuples;
- each approver's pinned declared failure domain;
- minimum approval and distinct-failure-domain counts;
- requester, beneficiary, and successor independence requirements;
- an optional delegation ceiling; and
- an optional one-operation ceremony.

Several signatures do not imply independence. Only distinct pinned failure
domains count toward `minimum_failure_domains`. Duplicate approvals from the
same exact principal conflict. Missing, unavailable, partitioned, expired,
revoked, replayed, or conflicting approval state fails closed.

If successor independence is required, no approval whose source plan is the
active predecessor can activate its successor. `validate_support_graph`
rejects self edges and bounded cyclic mutual-support graphs.

## Monotonic delegation and recovery

`DelegationEnvelope` contains action, resource selector, audience, time
window, time basis, and remaining depth. A child is valid only when:

- action and audience are equal;
- an exact resource remains exact, or a kind selector narrows to the same kind;
- `not_before_tick` does not move earlier;
- `expires_at_tick` does not move later;
- the time basis is unchanged; and
- remaining depth does not increase.

`validate_recovery_narrowing` applies the same relation. Recovery, emergency,
rollback, and restart do not create a more permissive authority path.

## Governance handles

A protected governance or root handle is unavailable to ordinary plans. An
administrative proposal naming one MUST also name a ceremony, and the policy
MUST pin that exact ceremony. The handle remains a descriptor reference; core
records and diagnostics never contain key or secret material.

## Plans and hosted compilation

Execution-plan schema 11 adds optional `administrative_subject` and
`containment` facts to `PlanAuthority`. Both are absent for ordinary effects.
Both are required for a marked administrative effect. The containment
execution identity participates in plan identity, and the portable validator
revalidates the complete proof at plan creation and use time.

Hosted administrative plans use `conduit.execution-plan/v4`; ordinary hosted
plans remain `conduit.execution-plan/v3`/plan schema 3. Compile-input v2 adds
optional containment documents without changing ordinary serialized inputs.
`conduct --check --compile-input INPUT PANEL` and the corresponding
`--explain` form validate the explicit snapshot. Missing independent approval
reports `CND-CTN-007` and names that proof rather than degrading to a generic
resolution failure.

## Bounded explanations and evidence

`ContainmentReasonNode` is caller-owned and limited to 16 nodes and depth 8.
Each non-root node names one prior parent at exactly the preceding depth.
`AdministrativeControlRecord` is fixed-size and may record request, denial,
approval, expiry, commit, execution, or revocation without payload or handle
material.

Stable rejection codes are `CND-CTN-001` through `CND-CTN-024`, as exposed by
`ContainmentReason::code`.

## Conformance

`conformance/c2/containment-v1.json` contains 33 independently dispatched
cases. They cover ordinary and exact external approval success; self-grant,
successor, cyclic, clone, and installer rejection; real threshold-domain
counting; replay across every subject field; stale/revoked/conflicting and
unavailable approval; every delegation dimension; governance ceremony
pinning; rollback; and recovery narrowing.

Cryptographic verification, key custody, human ceremony execution,
organizational meaning, and domain operation semantics remain provider or
domain responsibilities. A human click, signature, hardware key, multisig, or
realm root is not independently safe merely by its kind.
