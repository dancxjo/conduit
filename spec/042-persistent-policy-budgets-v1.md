# Persistent policy budgets v1

Status: normative seed for issue #93.

## Purpose and authority boundary

Plan resource budgets, queue bounds, pool maxima, and ordinary backpressure are
finite execution facts. They are not cumulative governance state. Replacing a
plan or beginning a new run, epoch, generation, realm, or host process MUST NOT
replenish authority to perform a protected administrative or hazardous effect.

`PersistentBudgetPolicy` defines an executor-neutral budget owned outside the
consuming workload. `PersistentBudgetLedger` is a bounded allocator-free
reference state machine. A host or administrative provider supplies the
durable atomic storage, monotonic time observation, and evidence persistence.
Core does not claim that an in-memory instance is durable.

The policy action and resource class remain domain-owned identifiers and
pinned descriptors. Core does not add a closed list of enrollments,
installations, firmware operations, physical effects, or network effects.

## Exact policy identity

Every policy identity covers:

- the exact policy, owner, subject, resource-class, persistence-profile, and
  optional renewal-authority descriptors;
- one realm, host, or site anchor;
- the protected action and monotonic time basis;
- current-stock, rolling-window, and lifetime limits;
- reservation lifetime and optional lease rule;
- audit identity, reservation-slot maximum, and evidence-event maximum.

At least one limit is finite. Rolling limits name both units and a non-zero
window duration. Removing a finite limit is an increase, not an omission.

The ledger is selected by the policy identity and anchor. `PolicyBudgetConsumer`
retains realm, plan, epoch, generation, and run provenance for correlation and
evidence, but none of those fields selects or replaces the ledger. Therefore a
new plan, rollback, retry, reboot, generation, or realm cannot obtain a fresh
counter for the same host/site policy.

Realm and host/site budgets MAY coexist. Every distinct plan binding must pass;
one binding never overrides or substitutes for another.

## Limits

Current stock counts committed subjects that remain live. Releasing a committed
subject reduces current stock only.

Rolling consumption counts commits in the current deterministic window. A
window advances only from the named monotonic time basis. Wall-clock text is
not an identity or correctness source.

Lifetime consumption counts every committed unit and is never reduced by
release, rollback, plan replacement, upgrade, or reboot.

Reservations count against every applicable limit before a protected effect.
Checked arithmetic failure is denial.

## Atomic transition protocol

The authoritative provider MUST atomically persist the complete transition
before acknowledging it:

1. `reserve` validates exact request identity, policy, action, time, lease,
   evidence capacity, limit capacity, and a free bounded slot.
2. `commit` consumes a live reservation and increments current, rolling, and
   lifetime counters before the protected effect is acknowledged.
3. `release` frees an uncommitted reservation or reduces current stock for a
   removed committed subject; it never refunds rolling or lifetime use.
4. `expire` releases only uncommitted reservations whose deterministic
   reservation deadline has passed.
5. recovery accepts only an exact-policy checkpoint whose counters,
   reservation count, and evidence remainder remain within policy.

Reservation identity is distinct from request correlation. Repeating the same
exact request/correlation returns the same reservation and repeating its commit
is idempotent. Reusing a correlation for different request facts fails.
Concurrent executors share the authoritative ledger; serializable reservation
means only one contender can reserve the final unit.

The reference ledger stores a fixed caller-selected reservation array no larger
than the policy maximum. Exhausting slots or evidence fails before mutation.

## Status projection and plan binding

`PolicyBudgetStatus` is a fresh, identity-bearing projection naming the exact
policy, ledger descriptor, durable checkpoint, sequence, counters, evidence
remainder, availability, time basis, and observation interval. It is used for
resolution and explanation. It MUST NOT be used as the source of truth for
recovery.

Effect requirements may name an optional `policy_budget_class`. Absence
preserves the earlier effect identity. Presence requires one or more
`PlanPolicyBudget` bindings whose exact resource class matches and whose action
equals the effect action.

Execution-plan schema 12 retains each policy, status, optional lease, required
units, and use-time recheck flag in the plan identity. Hosted governed plans use
`conduit.execution-plan/v5`. Plan v11 administrative containment and ordinary
plan v3 identities remain readable and unchanged.

An available status is validated at plan creation. When `check_at_use` is set,
it is validated again against the use-time observation. Unavailable ledgers
fail closed unless the exact policy permits an explicit finite offline lease.
A retention gap is never bypassed by a lease.

## Offline lease

An offline lease binds the exact policy, holder, renewal authority, monotonic
time basis, issuance, expiry, and offline disposition. Its duration cannot
exceed the policy maximum. Offline use is accepted only when both policy and
lease explicitly allow it.

Expiry is exclusive and renewal is a new exact authorization by the pinned
renewal authority. Reboot, reconnect, or continued absence never silently
renews a lease.

## Administrative increase

Decreases preserving exact owner, subject, anchor, action, resource class, and
time basis are monotonic. Any widened boundary, removed limit, increased
allowance, or longer reservation lifetime requires the independent
`AdministrativeProof` from the containment contract.

The proposal subject and validation context MUST both pin the new exact budget
descriptor in `AdministrativeSubject.budget`. The consuming workload cannot
replace or replenish the policy it consumes.

## Bounded recovery and compaction

`PolicyBudgetCheckpoint` contains fixed counters, evidence remainder, retention
floor, and the fixed reservation slots. A rebuildable status or audit
projection is not sufficient recovery input.

Its checkpoint identity covers the prior checkpoint, all counters, retention
floor, evidence remainder, and every non-empty reservation state. Recovery
rejects counter or reservation mutation that attempts to reuse the old
checkpoint identity. Authentication and atomic durability of that checkpoint
remain provider responsibilities.

Terminal correlations may be compacted only by advancing an explicit retention
floor and recording a new checkpoint. A request older than that floor fails
with a recovery-gap denial; it is never treated as new. No unbounded audit
history is required for current enforcement.

## Diagnostics and conformance

Stable reasons are `CND-PBG-001` through `CND-PBG-019`.
`CND-PBG-008` is persistent policy-budget denial and remains distinct from
`CND-PLN-006` plan-resource exhaustion. Hosted compile/check/explain surfaces
the persistent reason without relabeling it as an ordinary allocation failure.

`conformance/c2/persistent-budget-v1.json` contains 15 independently dispatched
cases. They execute the required cross-epoch lifetime, recovery, duplicate,
race, expiry, generation, realm-evasion, partition/stale, offline-expiry,
administrative-increase, evidence-first, stock/lifetime, retention-gap, and
coexisting-anchor scenarios.

Durable media, distributed serialization, cryptographic authentication,
organizational approval meaning, and the protected effect itself remain host or
domain responsibilities.
