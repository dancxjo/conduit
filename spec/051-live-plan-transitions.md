# Live plan transitions current form

Status: normative C5 contract. Requirement identifiers `TRN-001` through
`TRN-020` are exercised by
`conformance/c5/plan-transitions.json`.

## Boundary and identities

A transition is a bounded transaction between two immutable exact execution
plans. It is not an edit to either plan, a function-pointer swap, a registry
lookup during execution, or a presentation operation.

The following identities remain distinct:

- editable `.panel` source;
- old and candidate `ExecutionPlan` identities;
- monotonically increasing plan epochs;
- the transition contract and request;
- the independent decision and administrative authorization;
- the fresh host-resolution decision;
- persistent policy-budget status and reservation;
- whole-transition hazardous-effect closure;
- independent inhibit decision;
- immutable transition evidence; and
- Patchbay or other mutable presentation.

An external cord remains attached to one stable semantic instance/export path.
The stable path does not make two implementations, artifacts, private state
formats, security modes, or graph modes substitutable.

## Exact transition contract

`TransitionContract` pins:

- old and candidate plan identities and epochs;
- stable semantic subject;
- both implementation and artifact identities;
- cold, quiescent, or stateful replacement level;
- one domain-owned handoff boundary;
- optional exact state and retained-replay contracts;
- the complete required guarantee floor;
- every permitted optional-characteristic change;
- old, candidate, rollback, and exact summed overlap resources;
- finite in-flight, host-operation, state, replay, evidence, and tick bounds;
  and
- recovery attempt, cooldown, and hysteresis bounds.

The candidate epoch is greater than the old epoch, the plan identities differ,
and the candidate guarantee floor equals the required floor exactly. Semantic
contract, authority, sensitivity, delivery, memory, security, and
committedness hashes cannot be weakened under an implementation replacement.
A quality, latency, model-size, naturalness, timestamp, speaker, or capacity
change is legal only as a named optional characteristic already represented by
the stable contract.

The overlap reserve is the checked exact sum of old, candidate, and rollback
budgets. A smaller value is under-accounting and a larger value is not accepted
as an imprecise claim. Pool generations, timers, transports, checkpoints, and
evidence remain included through `PlanResourceBudget`.

## Replacement capability

An implementation manifest advertises its strongest honest capability:

```text
cold
quiescent(exact boundary, maximum ticks)
stateful(exact state contract, export bytes, import bytes, maximum ticks)
```

Cold transitions transfer no private state. Quiescent transitions require the
exact advertised boundary and deadline. Stateful transitions require the
exact state-contract identity and sufficient export, import, and time bounds.
Matching node contracts, ports, implementation names, or artifacts do not
upgrade this capability.

The fresh resolver decision carries the selected manifest's exact replacement
capability. Hosted old and candidate participants independently expose the
same capability. Admission and transaction construction reject either
mismatch before preparation.

State contracts are opaque and domain-owned. They include exact schema
identity, export/import bounds, sensitivity, and authority. Conduit copies
state only through a caller-owned bounded buffer and overwrites that buffer
before returning. It does not interpret speech, session, model, device, or
other private state.

## Admission

Before either generation is disturbed, hosted admission consumes the actual
contracts from specifications 025 and 041–045:

1. validate typed Resonance control request and decision envelopes;
2. require exact subject, correlation, cause, and optimistic old-plan epoch;
3. validate an independently authorized successor administrative effect;
4. validate a complete fresh candidate resolver binding and replacement
   capability;
5. validate fresh persistent-budget status and atomically reserve the
   authoritative durable ledger;
6. analyze whole-transition effect closure across old, candidate, and
   rollback generations;
7. validate required hazardous-host/inhibit facts against the selected host;
   and
8. expose separate nonzero proof identities to the portable controller.

The authoritative ledger is changed only after every validator succeeds.
Forged, stale, replayed, revoked, wrong-epoch, self-supporting, unavailable,
ambiguous, or mismatched facts fail closed. Resolution performs no
provisioning, installation, artifact fetch, permission acquisition,
enrollment, federation, or persistence expansion.

Persistent reservations, committed stock, lifetime use, lease state, sequence,
and checkpoints survive rollback/retry and later epochs. A released failed
reservation does not erase its durable record or reset the controller's
attempt/cooldown state.

## Transaction and mutation ordering

The portable controller admits only:

```text
requested
  -> reserved
  -> prepared
  -> barrier
  -> draining
  -> transferring | discontinuous
  -> rebinding
  -> committed
  -> retiring
  -> completed
```

A pre-commit phase may instead enter deterministic rollback. A host failure
while restoring the old boundary is terminal. Retirement is irreversible:
after old retirement begins, an error cannot manufacture rollback evidence.

Every hosted lifecycle mutation has a read-only core preflight at the same
tick. Evidence exhaustion, deadline expiry, wrong phase, wrong boundary, or
known contract mismatch is therefore rejected before the backend changes.

Preparation starts the candidate without live traffic. The barrier:

1. stops new admissions to the old generation at the exact boundary; and
2. atomically routes new boundary admissions to the prepared candidate while
   already-admitted work remains pinned to the old generation.

The active authoritative epoch remains old until commit. Candidate and old
events during overlap name their own exact plan epoch; a projection must not
collapse this multi-epoch history into one mutable “current mode” record.

Drain observations account cumulatively and exactly for every value live at
reservation as remaining, drained, rejected, or lost, and every pending host
operation as remaining, completed, or cancelled. The sum must equal the
reserved initial count. Rebind is forbidden while any work remains unless the
contract enters an explicit permitted discontinuity. Output-full and
input-in-flight cases use the same accounting; pressure does not authorize a
hidden queue or loss.

After state transfer/replay and exact drain, the router atomically finalizes
the stable endpoint binding. Persistent budget commits before the portable
controller changes the active epoch. Old retirement and transaction
completion are separately evidenced.

## Replay and discontinuity

Replay names an exact Resonance stream, stream epoch, first cursor, item/byte
bounds, duplicate policy, and gap policy. The provider is explicit and owns
retention; the transition owns no private event history. Each item has an exact
cursor, byte length, redelivery flag, and gap observation.

Wrong streams, epochs, cursors, excess items/bytes, undeclared duplicates, and
overflow fail before commit. A retention gap deterministically rejects,
requires rollback, or declares discontinuity according to the pinned policy.
Replayed and duplicate counts are evidence. Checkpoint, state transfer,
retained replay, and implementation handoff remain distinct claims.

## Rollback, recovery, and containment

Before commit, rollback aborts the candidate, atomically restores new
admissions to the old generation, restores the old participant, releases the
durable reservation, and then records the old epoch as authoritative. If any
host restoration step fails, the transition becomes terminal rather than
claiming success.

Recovery is bounded by cumulative attempts, cooldown, hysteresis, and a
transition deadline. Retry returns to `requested` with zero transient usage
and no retained admission proof, but it does not reset attempts or durable
policy state. Same-epoch restarts owned by specification 049 do not become
plan transitions; an undeclared implementation/artifact/host/topology change
requires this transaction.

Whole-transition effect closure is evaluated before prepare. Two individually
acceptable generations are rejected when coexistence creates a toxic
combination or exceeds the exact overlap reserve. Hazardous transitions bind
the independent inhibit plane to the candidate host. Assertion, stale
observation, authority loss, host-report expiry, or ledger loss causes the
policy-defined rollback, discontinuity, or terminal result; it never becomes
an implementation-controlled approval.

## Domain integrations

Domain-specific types and boundaries stay outside `conduit-core`.

- A Tongues TTS replacement uses an opaque utterance boundary and quiescent
  capability. A completed utterance remains on the old generation; the next
  utterance is admitted to the prepared candidate.
- A Tongues streaming-ASR replacement uses an opaque segment boundary plus an
  exact state contract and bounded retained-audio replay, or an explicitly
  permitted discontinuity. Required committed transcripts cannot degrade to
  unstable partial hypotheses.
- The HTTP profile supplies a concrete in-memory generation participant.
  Existing old requests continue through the real bounded HTTP backend after
  its admission barrier, new requests enter the prepared generation, exact
  drain reaches zero, and the generic transaction commits and retires old.
  HTTPS, authenticated transport, and session guarantees remain in the
  required floor.
- Replicated pools use specification 050's exact old/candidate/rollback
  generation reserve and drain/rollback primitives inside the same overlap
  budget. A transition cannot borrow cleanup slots.

These are reference witnesses for the generic mechanism, not a Conduit-owned
speech, model-provider, HTTP semantic, or session-state taxonomy.

## Evidence

Every evidence record includes sequence, tick, transition identity, old and
candidate plan/epoch, currently authoritative epoch, stable subject, phase,
kind, optional exact cause and boundary, exact usage/disposition counts, and
the separate admission proof identities.

Required kinds cover request, reserve, candidate prepare, admission barrier,
drain, state transfer, replay/discontinuity, endpoint rebind, commit,
retirement, completion, rollback start/result, recovery suppression, and
terminal result. Diagnostic strings are presentation only and never control
decisions.

## Stable reasons

- `CND-TRN-001` unsupported transition schema
- `CND-TRN-002` transition identity mismatch
- `CND-TRN-003` invalid or unbounded contract
- `CND-TRN-004` immutable epoch requirement violated
- `CND-TRN-005` required guarantee weakened
- `CND-TRN-006` replacement capability unsupported
- `CND-TRN-007` state contract/capability mismatch
- `CND-TRN-008` replay contract mismatch
- `CND-TRN-009` overlap, work, or disposition bound exceeded
- `CND-TRN-010` evidence capacity exhausted
- `CND-TRN-011` illegal phase or live work at rebind
- `CND-TRN-012` handoff boundary mismatch
- `CND-TRN-013` stale active plan epoch
- `CND-TRN-014` admission proof absent
- `CND-TRN-015` recovery attempt limit
- `CND-TRN-016` cooldown or hysteresis active
- `CND-TRN-017` transition deadline exceeded
- `CND-TRN-018` state representation exceeds bound
- `CND-TRN-019` retained replay gap
- `CND-TRN-ADM-001` control request/decision mismatch
- `CND-TRN-ADM-002` administrative/budget subject mismatch
- `CND-TRN-ADM-003` fresh exact resolver binding absent
- `CND-TRN-HOST-001` concrete generation binding mismatch
- `CND-TRN-HOST-002` generation lifecycle failure
- `CND-TRN-HOST-003` stable-boundary router failure
- `CND-TRN-HOST-004` retained provider failure
- `CND-TRN-HOST-005` state buffer/bound violation
- `CND-TRN-HOST-006` replay binding/sequence/bound violation
- `CND-TRN-HOST-007` persistent reservation mismatch
- `CND-TRN-HOST-008` rollback restoration failure
- `CND-TRN-HOST-009` admission authority/report/budget/inhibit horizon expired

Specifications 025 and 041–045 retain their own stable reason families for
control, administrative containment, durable budgets, effect closure, safe
distribution, and inhibit validation.

## Normative requirements

| ID | Obligation |
|---|---|
| TRN-001 | Keep both execution plans immutable and create a distinct increasing candidate epoch |
| TRN-002 | Keep stable semantic boundary, implementation, artifact, plan, transition, evidence, and presentation identities distinct |
| TRN-003 | Enforce exact cold, quiescent, and stateful manifest/runtime capabilities |
| TRN-004 | Preserve the complete semantic, authority, sensitivity, delivery, memory, security, and committedness floor |
| TRN-005 | Name and evidence every permitted optional-characteristic change |
| TRN-006 | Reserve exact old plus candidate plus rollback resources and all finite work/state/replay/evidence bounds before prepare |
| TRN-007 | Consume fresh Resonance control and independent #94 successor authorization |
| TRN-008 | Consume a fresh exact resolver decision without provisioning or acquisition |
| TRN-009 | Consume authoritative persistent #95 budget state without reset through rollback/retry |
| TRN-010 | Evaluate #96 whole-transition effect closure including temporary overlap |
| TRN-011 | Preserve #98 independent inhibit facts and fail closed on missing, stale, or mismatched state |
| TRN-012 | Preflight every hosted mutation and deterministically prepare, barrier, drain, transfer/replay, rebind, commit, retire, or roll back |
| TRN-013 | Route new boundary admissions to the prepared candidate while exact old work drains |
| TRN-014 | Give every initially live value and operation one bounded explicit disposition before rebind |
| TRN-015 | Use only an explicit bounded #79 replay provider and enforce cursor, duplicate, and gap policy |
| TRN-016 | Overwrite caller-owned state scratch and never interpret domain-private state |
| TRN-017 | Bound attempts, cooldown, hysteresis, deadline, and terminal restoration behavior |
| TRN-018 | Emit complete structured multi-epoch transition and containment evidence |
| TRN-019 | Execute opaque Tongues ASR/TTS and concrete HTTP request-generation witnesses above core |
| TRN-020 | Consume replicated-pool generation overlap and supervision primitives without hidden capacity |
