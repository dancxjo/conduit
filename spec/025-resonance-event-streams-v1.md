# Resonance typed event streams version 1

Status: C4 normative portable contract

Resonance contract version: 1

ExecutionPlan schema version: 5

Depends on: specifications 007, 008, 010, 011, 012, 017, 023, and 024

## Boundary

Resonance is Conduit's opt-in temporal/event dimension. Nodes, typed ports,
bounded cords, immutable plans, runs, and host implementations remain the
execution model. Resonance adds explicit typed recording, publication,
retention, subscription, replay, and projection contracts. It is not another
runtime, graph ontology, log convention, or domain framework.

The following remain distinct:

- live cord values governed by ordinary `FlowPolicy`;
- normative executor evidence frozen by specification 012;
- domain-owned typed occurrences;
- typed control requests, decisions, corrections, and transitions; and
- rebuildable mutable projections.

A cord value never becomes an event implicitly. Recording/publication is a
separate plan-v5 `PlanEventStream`. Logs, UI messages, and projection rows are
not normative evidence.

## Compatible shared envelope

`ResonanceEnvelope` extends rather than replaces `ExecutionEvent` v1. The
normative-evidence adapter validates the original event and preserves its
event ID, run, plan identity/epoch, subject, sequence, observer sequence,
domain time, correlation, idempotency, causation, derivation, supersession,
retraction, payload type/material policy, sensitivity, and authority. The
frozen event identity remains valid and unchanged.

The additive envelope names stream, producer, event class, provenance,
integrity, and optional `corrects`. Event classes are normative evidence,
domain, and control. Domains own payload `TypeContractRef` meanings; core owns
only generic validation and bounds.

Corrections, supersessions, and retractions append new events referencing
prior IDs. Recorded history is immutable. Observation order on different
hosts is local; causation is explicit and no universal clock or total
distributed order is inferred.

Payloads are inline-public, content-addressed, redacted, or absent. Protected
inline material is invalid. Redaction preserves envelope identity fields,
type, shape policy, authority decision, integrity, and provenance without
revealing content.

## Plan-visible stream and provider

Plan v5 pins publisher, complete `EventStreamContract`, resolved provider
capabilities, and exact resource allocation. Identity covers:

- stream ID, class, payload type, authority, sensitivity, and provider pin;
- ephemeral, retained-ring, checkpoint-associated, or durable-append policy;
- event/byte/checkpoint/time bounds and terminal sealing;
- coupled/isolated subscriber `FlowPolicy`, delivery claim, publisher,
  subscriber, pending-operation, and projection bounds;
- resolved provider durability, retention, cursor, integrity, redaction, and
  capacity claims; and
- memory, storage, CPU, timer, transport, checkpoint, and evidence allocation.

V1-v4 plan identities are frozen. Adding stream facts to an old schema is
rejected; migration re-lowers exact source to v5.

Durability is a resolved capability with integrity and bounded flush, not a
synonym for queued or eventually written. An embedded retained ring honestly
claims retained but not durable. Provider-owned buffers, indexes, retries,
cursors, flush state, and subscriber queues fit the exact allocation.

## Publication, retention, and crash recovery

Append has a defined commit point. Crash before commit discards a partial
write; crash after commit replays the committed event. Recovery never mutates
the prior event or fabricates a terminal record. A sealed stream rejects new
append with `CND-RSN-004`.

Coupled subscribers use one finite queue contract and may backpressure the
publisher. Isolated subscribers have independent finite queues/policies.
Optional retained-ring overflow evicts only under the declared policy and
advances `first_available`; consumers receive a gap, never guessed history.
Normative terminal/loss/authority/transition evidence required by policy is
not silently sampled. Sampling optional telemetry emits its own summary/gap.

## Subscription and replay

`SubscriptionContract` pins stream, start position, queue policy,
acknowledgement window, and cancellation bound. Start positions are head,
tail, exact cursor, checkpoint, or provider index. Unsupported positions fail
provider resolution.

Replay preserves stream cursor order and exposes gaps, duplicates,
redelivery, and terminal sealing. Delivery is at-most-once or at-least-once.
There is no universal exactly-once claim. Idempotency can make redelivery safe
at one cooperating consumer boundary; non-idempotent consumers must observe
duplicates and choose explicit policy. Resynchronization starts from a
reported available cursor/snapshot, never an inferred missing event.

## Projection

A projection is an ordinary typed node/composite consuming an event stream.
Its contract pins versioned logic, snapshot type, stream, maximum state bytes,
maximum rebuild events, cursor, and gap policy. Snapshots are rebuild aids,
not authoritative history. Deterministic rebuild consumes the same permitted
events in cursor order. A logic-version change changes projection identity and
requires rebuild/migration. Patchbay may inspect permitted streams and
projections but is neither store nor run model.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-RSN-001` | invalid envelope, relationship, payload, or identity |
| `CND-RSN-002` | zero, overflowed, or exceeded stream/subscriber/replay bound |
| `CND-RSN-003` | provider cannot honor retention, durability, security, cursor, or capacity |
| `CND-RSN-004` | append attempted after terminal sealing |

## Requirements

- RSN-001: preserve cord, evidence, domain, control, and projection boundaries.
- RSN-002: extend frozen ExecutionEvent v1 compatibly.
- RSN-003: keep domain payload semantics outside core.
- RSN-004: make publication and all storage plan-visible and finite.
- RSN-005: append corrections/retractions without mutation.
- RSN-006: use explicit local order and causation, never a universal clock.
- RSN-007: resolve retention/durability against honest provider capabilities.
- RSN-008: define append commit and partial-write recovery.
- RSN-009: bound coupled/isolated subscription pressure independently.
- RSN-010: expose replay gaps, duplicates, redelivery, and sealing.
- RSN-011: make cursor/checkpoint replay explicit and capability-gated.
- RSN-012: preserve required terminal/loss/authority/transition evidence.
- RSN-013: make projections deterministic, rebuildable, versioned, and bounded.
- RSN-014: preserve v1-v4 plan identities and add exact plan-v5 stream identity.
- RSN-015: keep transport-native keys/topics outside semantic stream identity.
- RSN-016: support embedded retained profiles without false durability.

The normative fixture is `conformance/c4/resonance-v1.json`.
