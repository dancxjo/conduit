# Durable finite jobs and checkpoints current form

Status: C4 normative portable contract

Checkpoint envelope schema marker: `0`

ExecutionPlan schema marker: `0`

Depends on: specifications 008, 011, 012, 017, 022, 023, 024, and 025

## Identity and evidence boundary

A durable job distinguishes job, run, attempt, work-unit, work-lease,
checkpoint, output artifact, validation decision, accepted result, and
idempotency identities. Retry and resume preserve the logical job/work unit
and applicable idempotency context but allocate a new attempt and lease. Late
evidence from an expired attempt remains immutable; it never overwrites the
new attempt. A checkpoint references its source attempt and lease context; it
never becomes the resumed attempt.

Progress is immutable attempt evidence with a monotonic evidence sequence.
A mutable status projection may be rebuilt from evidence but is not the sole
record. Progress and terminal outcomes use the plan-selected durable
normative-evidence stream from specification 025. The stream cursor and epoch
are not checkpoint identity, and the mutable “current percent” projection is
not evidence. Progress MUST remain below the known positive total until the
declared result commit is observed. Only then may attempt-executed evidence
record the total. A crash never manufactures the missing event.

## Finite attempt contract

`JobContract` pins positive total work, maximum attempts, retry backoff,
attempt deadline, checkpoints and checkpoint bytes, a named lease clock and
renewal maximum, duplicate policy, delivery claim, commit boundary, checkpoint
provider, durable evidence stream, restart policy, optional domain-validation
policy, and bounded cancellation-checkpoint policy. `WorkLease` pins its
holder identities, clock, issuance/expiry, and renewal ordinal. Expired work
cannot commit, checkpoint, or report progress.

Retry creates a new attempt and lease within the finite attempt, backoff, and
deadline budget. A non-checkpointable job declares restart-from-beginning and a
positive lost-work bound; it cannot silently claim resumability.

Cancellation either aborts to a terminal cancellation or enters a final
checkpoint phase with a finite tick bound. Deadline expiry deterministically
aborts. An incomplete/failed checkpoint is not selectable. Abort during
checkpoint leaves no resumable envelope.

## Delivery and duplicate behavior

Every boundary declares `at-most-once` or `at-least-once`.
`transactional-exactly-once` is valid only with a pinned transactional
source/sink boundary and acknowledgement evidence. This is a claim about that
one named boundary, not universal execution. An idempotency key is metadata
used by a cooperating boundary; it does not make unrelated effects
exactly-once.

After a crash before durable commit, recovery creates a new attempt and
retries under the finite policy. After a sink commit but before terminal
evidence, the same idempotency key discovers the committed result and appends
the missing attempt/terminal evidence according to duplicate policy. Policies
are reject, return-committed, or safe retry with the same key.

The commit proof pins the domain-owned boundary descriptor, idempotency key,
distinct result identity and digest, commit evidence, and any acknowledgement
evidence. A process
exit, progress record, checkpoint, or lost acknowledgement never fabricates
an external effect. General resource-lease, cleanup, and compensation profiles
remain owned by the later effect-boundary contract; this contract only records
the exact commit proof a job relied upon.

## Execution, validation, and acceptance

Executor completion means one attempt produced and durably committed an
output. It does not assert domain validity or canonical acceptance.
`ResultValidationPolicy` optionally pins a domain-owned validator,
equivalence/tolerance descriptor, optional homogeneous numerical constraint,
finite result maximum, quorum, and deadline. Validation decisions are
immutable, name the exact output identity/digest and applicable homogeneous
constraint, and distinguish accepted, rejected, inconclusive, conflicting,
and late outcomes. Only an on-time accepted quorum may name a canonical result.
Replication is explicit finite policy or a domain composite, never hidden
executor magic.

## Checkpoint envelope and resume

The integrity-protected envelope pins:

- checkpoint status, job/run/work-unit, source-attempt/source-lease, and sequence;
- checkpoint provider, durable evidence stream, stream epoch, and exact cursor;
- exact plan identity;
- implementation, artifact-set, configuration, type-contract-set, logical
  template, and correlation hashes;
- migration version;
- bounded content-addressed node, cord, source-offset, and committed-result
  state references; and
- a canonical integrity digest over every field and state reference.

Resume validates complete status, syntax, nonempty bounded state, integrity,
selected checkpoint/provider/stream epoch/cursor, job identity, a new attempt,
and every compatibility hash before importing state. Exact match resumes
without migration. Any semantic mismatch fails `CND-JOB-004` unless one
explicit pinned migration names the exact source and target compatibility
hashes and advances the migration version. Best-effort loading and omitted
hashes are forbidden.

Event append and checkpoint publication have independent prepare/commit crash
boundaries. Recovery discards a pre-commit partial event or checkpoint,
replays a committed event, and selects only a committed complete checkpoint.
Neither commit implies the other; resume pins both the selected checkpoint and
the authoritative event cursor.

Source offsets and queued accepted values are distinct state references.
Restoration preserves their exact state contracts, owners, content digests,
and byte charges. No replay/checkpoint buffer exists outside the plan and
implementation budgets.

A checkpoint is non-portable by default. Replacement through a different
implementation/artifact requires the later state-transition contract, not a
claim that ordinary resume implies substitution compatibility.

## Exact plan and provider boundary

current plan schema adds `jobs` separately from cords, event streams, source, and
evidence. A plan job names its owner, full semantic contract, observed
checkpoint-provider capabilities, and exact memory, storage, timer, checkpoint,
and evidence allocations. Its progress stream is a separate current plan
`event_streams` entry and MUST be durable append, at-least-once,
integrity-capable, non-lossy for required terminal evidence, and bounded.

Checkpoint providers advertise durable/integrity/migration support and maxima
for checkpoint count, bytes, state references, and pending operations. The
plan allocates the maximum retained checkpoint bytes and creation memory; an
incapable provider fails `CND-JOB-016`. Plans current through current cannot carry job
facts and retain their current identities. Re-lowering a durable job creates a
current plan identity.

## Stable diagnostics

| Code | Meaning |
|---|---|
| `CND-JOB-001` | malformed checkpoint envelope or collapsed identity |
| `CND-JOB-002` | checkpoint scratch/byte bound exceeded |
| `CND-JOB-003` | checkpoint integrity mismatch |
| `CND-JOB-004` | plan/implementation/artifact/config/type/template/correlation incompatibility |
| `CND-JOB-005` | explicit migration is absent or mismatched |
| `CND-JOB-006` | invalid finite job or delivery contract |
| `CND-JOB-007` | illegal attempt/checkpoint transition |
| `CND-JOB-008` | progress or completion precedes durable commit |
| `CND-JOB-009` | durable commit idempotency mismatch |
| `CND-JOB-010` | work lease expired |
| `CND-JOB-011` | immutable evidence sequence overflow |
| `CND-JOB-012` | attempt/cancellation deadline exceeded |
| `CND-JOB-013` | lease identity or bounded renewal invalid |
| `CND-JOB-014` | job evidence and Resonance envelope mismatch |
| `CND-JOB-015` | domain validation/quorum decision invalid |
| `CND-JOB-016` | evidence or checkpoint provider incapable |

## Requirements

- JOB-001: keep every durable identity family, including lease and acceptance, distinct.
- JOB-002: bind progress and terminal outcomes to immutable Resonance attempt evidence.
- JOB-003: bound attempts, leases, checkpoints, bytes, cancellation, and retry.
- JOB-004: never record total completion before durable commit.
- JOB-005: make duplicate execution and domain validation explicit and policy-controlled.
- JOB-006: state delivery honestly and scope transactional exactly-once to a named boundary.
- JOB-007: integrity-protect every checkpoint field and state reference.
- JOB-008: require exact resume compatibility or explicit pinned migration.
- JOB-009: restore source offsets and queued values without hidden storage.
- JOB-010: reject partial, failed, corrupt, oversized, or expired state.
- JOB-011: keep ordinary checkpoint resume distinct from replacement state.
- JOB-012: replay terminal evidence without fabricating success.

The normative fixture is `conformance/c4/durable-job.json`.

## Migration

Checkpoint schema current and job contract current are new. There is no implicit
conversion from a mutable status row, an unversioned state blob, or a
current plan–current runtime. Hosted tooling must re-lower an authored durable job to
current plan with resolved providers and budgets. Existing current plan through current plan,
ExecutionEvent current, and Resonance current identities are unchanged.
