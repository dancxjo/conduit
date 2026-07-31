# Resource lease and effect commit contract current form

Status: implemented portable and hosted contract

Depends on: specifications 006, 010, 011, 017, 029, 037, 049, 050, and 051

## Boundary

An authority grant permits an effect and a resource binding identifies a
selected host resource. Neither fact reserves finite capacity, proves that the
same run still owns it, defines when a domain effect commits, or makes a retry
exactly once. This specification adds those missing boundaries without moving
file, process, socket, database, robot, or other domain semantics into
`conduit-core`.

current plan schema (`conduit.execution-plan`) pins a resource lease beside an
exact resource binding and an effect commit profile beside each authority.
Changing either changes plan identity. Older schemas reject these fields.

## Finite resource lease

A `ResourceLeaseContract` pins:

- lease, resource-binding, holder-instance, run, and epoch identity;
- domain scope and exclusive, shared-read, or finite shared-holder policy;
- an exact reservation that fits within the holder node's allocation;
- time basis, issue and expiry ticks, revocation grace, and cleanup bound;
- maximum operations and evidence events;
- a pinned cleanup-escalation contract; and
- foreign retention as none, finitely bounded, observed-only, or unsupported.

The lease is checked at every operation use together with fresh authority and
the existing host-operation profile. Holder, run, epoch, resource, clock,
expiry, revocation, operation, or evidence mismatch fails before provider
mutation. Revocation and expiry deny new use but do not fabricate cleanup.

Release is generation-sequenced. A stale release cannot free a newer
reservation. Cleanup has a finite deadline and an explicit failed/escalation
disposition. Cancellation is not complete while effect or lease cleanup is
pending.

## Domain commit profile

An `EffectCommitProfile` pins:

- the exact operation and lease;
- a domain-owned commit-boundary descriptor;
- no-idempotency, same-key/same-effect, or reconcile-before-retry behavior;
- the policy for an unknown commit and host discontinuity;
- a pinned cleanup contract; and
- finite attempt and evidence bounds.

Conduit does not assert global exactly-once execution. Before the commit
boundary a failure may be reported as not committed. After commit but before
acknowledgement, success is forbidden until the provider acknowledges,
reconciles, or follows the exact same-key retry contract. A lost
acknowledgement therefore remains `commit-unknown`, not success.

## Hosted witnesses

The deterministic hosted backend injects failures before commit, after commit
but before acknowledgement, and during cleanup. It reserves the attempt's
complete evidence allowance before invoking the provider.

Linux witnesses operate only on handles explicitly supplied by the host:

- file commit is successful write plus `sync_data`;
- process-launch commit is successful `spawn`, distinct from child completion;
- local-socket commit is kernel acceptance of all bytes, not peer processing;
- process escalation is finite kill plus wait.

These witnesses demonstrate real kernel boundaries. They do not discover
ambient resources, prove remote durability, or upgrade a domain profile to
exactly once.

## Diagnostics

`CND-LSE-001` through `CND-LSE-021` distinguish invalid contracts and
identities, holder/run/epoch/resource/time failures, expiry/revocation,
sharing and operation bounds, stale release, cleanup, evidence exhaustion,
unknown commit, forbidden retry, host loss, and illegal lifecycle
transitions. Exact-plan diagnostics retain the failing resource or authority
collection and index.

## Requirements

- **LSE-001:** Every live resource use is bound to an exact finite
  holder/run/epoch lease and reservation.
- **LSE-002:** Fresh authority, host-operation constraints, and lease facts are
  checked before every provider mutation.
- **LSE-003:** Revocation, expiry, release, cancellation, and cleanup have
  finite deterministic dispositions; stale release fails closed.
- **LSE-004:** Evidence and foreign retention are bounded or truthfully
  classified before execution.
- **CMT-001:** Every planned effect pins a domain-owned commit and cleanup
  profile.
- **CMT-002:** Success requires commit acknowledgement; lost acknowledgement
  remains unknown until an allowed retry or reconciliation resolves it.
- **CMT-003:** Idempotency claims are local to the pinned domain contract;
  Conduit never claims global exactly once.
- **CMT-004:** Deterministic faults and real hosted witnesses cover
  before-commit, after-commit, host-loss, forced-cleanup, file, process, and
  socket boundaries.

## Conformance

Positive cases cover exact-plan round trip and identity, use-time admission,
acknowledged commit, same-key retry, bounded cleanup, and Linux file, process,
and socket witnesses.

Negative and boundary cases cover wrong holder/run/epoch/resource, expiry,
revocation, operation and evidence exhaustion, missing commit profiles,
unbounded reservation, stale release, lost acknowledgement, forbidden retry,
cleanup timeout, foreign retention misstatement, and schema downgrade.

Patchbay and Tour project only exact plan facts and runtime dispositions. They
do not infer a lease or manufacture successful cleanup.
