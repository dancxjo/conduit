# Immutable ExecutionEvent evidence and provenance version 1

Status: stable

ExecutionEvent schema version: 1

NDJSON representation version: 1

Specification
[`017-port-groups-correlation-v1.md`](017-port-groups-correlation-v1.md)
normatively specializes the allocators, scopes, lifetimes, sensitivity, and
propagation of these correlation fields without changing event schema 1.

## Evidence boundary

An `ExecutionEvent` is one immutable normative observation linked to an exact
run and execution plan. It is distinct from:

- application or process logs;
- mutable “current state” projections;
- metrics aggregates;
- `.panel` source and exact `ExecutionPlan`;
- implementation-private tracing; and
- Patchbay tables, labels, layout, filtering, and other presentation.

Logs and projections may be derived from events. They do not become evidence
merely because they contain similar text. Corrections and retractions append
new events; no recorded event, payload, identity, or sequence is rewritten.

The existing `LifecycleEvent`, `FlowEvent`, and `AuthorityEvent` types are
local semantic observations. A recorder wraps their exact facts in the common
execution-event envelope. Their local sequences remain useful payload facts
and never substitute for the run recorder sequence.

## Immutable envelope

Version 1 contains:

- schema version and canonical event identity;
- stable event ID and run ID;
- exact `ExecutionPlan` semantic hash;
- append sequence assigned by the run recorder;
- run-recorder ID, observer ID, and observer-local sequence;
- optional logical template path and exact expanded subject path;
- closed core family plus open detail ID;
- observed time and optional domain/event time;
- immutable correlation context;
- causal, derivation, supersession, and retraction relationships;
- exact terminality; and
- no payload, bounded public inline payload, content reference, or structural
  redaction.

Every event in one replay has the same run ID, exact plan hash, and run
recorder. Event ID and canonical identity are unique. Complete replay begins
at sequence zero and increments without gaps or overflow. Each observer's
first local sequence is zero and increments independently when that observer
next appears.

The final event may be terminal. No event follows a terminal event in the same
run stream.

## Paths and correlation

`subject` is the exact expanded instance, cord, run, resource, or authority
path observed by the event. `logical_template`, when present, is equal to or a
boundary-safe ancestor of that path. This preserves both logical replicated
template identity and the expanded attempt path.

`EventCorrelation` carries independent optional identities for:

- request;
- request/reply exchange;
- session and numeric epoch;
- work unit;
- attempt or retry;
- general correlation;
- idempotency;
- checkpoint; and
- transport.

The context is copied unchanged across applicable requests, replies, sessions,
attempts, checkpoints, transports, and replicated instances. None is allocated
from wall time or scheduler order, and none is silently collapsed into another
identity. Event and run identities likewise cannot be derived solely from a
wall-clock timestamp.

## Ordering, time, and causation

Three concepts remain separate:

1. append `sequence` is the run recorder's total order for one run;
2. `observer_sequence` retains each contributing observer's local observation
   order; and
3. observed or domain timestamps retain explicitly named time bases.

`EventTime` records `monotonic`, `wall`, or `domain` kind, stable basis ID, and
signed tick. Domain/event time appears only in the optional domain-time field.
Wall and domain ticks can move backward in append order. No timestamp creates
causality and unsynchronized hosts do not claim a universal clock.

`caused_by` names one direct event. `derived_from` is a bounded set of at most
16 direct input events, sufficient to reconstruct enabled cross-node
derivations. A complete replay requires every causal and derivation reference
to resolve, but a referenced remote event may appear later in append order
when network delivery was inverted.

Supersession and retraction targets must already exist earlier in append order.
A `correction` event has exactly one `supersedes` target; a `retraction` event
has exactly one `retracts` target. Other families have neither.

## Core families and domain extension

Core family names are:

- lifecycle;
- cord occupancy and pressure;
- value accepted, rejected, dropped, and coalesced;
- cancellation and terminal;
- resource and authority change;
- checkpoint and progress;
- derivation;
- domain;
- correction; and
- retraction.

The family controls common validation and replay behavior. `detail` is a
stable open identifier naming the exact lifecycle transition, FlowPolicy
decision, authority result, resource observation, or domain-owned event.
Domains use their own TypeContract and detail IDs; Conduit core does not gain
domain enums.

A dropped disposable value ordinarily yields an ordered pressure event, loss
event naming the value/cord and exact policy, and eventual pressure-cleared
event. Those are separate facts. A corrected hypothesis appends a domain
correction referencing the prior event; it does not mutate the prior payload.

## Typed payload and redaction

`EventPayload` has four structural modes:

| Mode | Value material | Required facts |
|---|---|---|
| none | no | no payload |
| inline-public | yes, public only | exact TypeContract and bounded bytes |
| reference | content-addressed elsewhere | TypeContract, digest, sensitivity, shape, recording authority for protected data |
| redacted | impossible to carry | TypeContract, protected sensitivity, policy-permitted shape, reason |

Inline bytes are possible only in the `InlinePublic` variant and must fit the
recorder's exact `max_inline_payload_bytes`. Protected values use an authorized
content reference or `Redacted`; no generic inline variant can be mislabeled.

A protected reference names the exact recording-authority ID. A redacted
payload has no byte or digest field and ordinary Debug can reveal only type,
sensitivity, allowed shape, and reason. `EvidencePolicy` independently controls
whether byte length and item count may survive redaction. Disallowed metadata
causes rejection rather than silent leakage.

This is the specification 010 sensitivity model at the evidence boundary. The
envelope itself, causation, and presence survive redaction.

## Canonical event identity

Event identity is the specification 003 canonical semantic hash of
`conduit/execution-event` schema 1. The identity field itself is excluded.
Every other normative envelope, correlation, relationship, terminal, type,
shape, digest, and payload-byte fact participates. `derived_from` is a
canonical set, so its source iteration order does not alter identity.

NDJSON whitespace, JSON object member order, escaping choice, streaming chunk
boundaries, log decoration, and presentation are not identity inputs. Changing
any normative event fact produces a different hash.

## Append-only validation and replay

`validate_execution_event` checks schema, portable IDs and paths, template
ancestry, named time bases, correlation IDs, bounded unique derivations,
correction shape, terminality, payload policy, structural redaction, and
canonical identity.

`validate_event_stream` additionally checks:

1. common run and plan;
2. gap-free append sequence and independent observer sequences;
3. unique event IDs and hashes;
4. terminal event last;
5. complete causation and derivation references; and
6. prior targets for correction and retraction.

It never sorts by timestamp. Replaying the frozen stream yields the same
ordered immutable events and identities; mutable projections may be rebuilt
but are not the record.

## Hosted NDJSON representation

`conduit-runtime` supplies:

- `OwnedExecutionEvent`, the allocator-aware in-memory record;
- `encode_event_ndjson`, one JSON object per line with a final newline;
- `decode_event_ndjson`, which rejects blank records; and
- `OwnedExecutionEvent::as_event`, which borrows into the allocator-free core
  form using caller-owned derivation scratch.

Hashes and artifact digests use lowercase `sha256:` text. Inline public bytes
are a JSON byte array. Payload and terminality variants are explicitly tagged.
Decode rejects malformed JSON and unknown fields. Borrowed conversion rejects
malformed IDs, paths, enum spellings, hashes, digests, and insufficient scratch
before core validation.

`conformance/c2/execution-event-v1.ndjson` is the frozen streaming fixture. It
contains public inline data, cross-recorder causation with timestamp inversion,
structurally redacted loss, and a final terminal event. Encoding, decoding,
owned re-encoding, borrowed conversion, semantic hashing, and replay validation
round-trip exactly.

## Diagnostics and fixtures

| Code | Meaning |
|---|---|
| `CND-EVD-001` | unsupported event schema |
| `CND-EVD-002` | malformed envelope, ID, path, time, correlation, or payload type |
| `CND-EVD-003` | canonical event identity mismatch |
| `CND-EVD-004` | public inline payload exceeds policy |
| `CND-EVD-005` | protected recording or redaction violates sensitivity policy |
| `CND-EVD-006` | derivation count exceeds the allocator-free bound |
| `CND-EVD-007` | duplicate event ID or canonical identity |
| `CND-EVD-008` | run or exact-plan linkage changed within a replay |
| `CND-EVD-009` | append or observer-local sequence is invalid |
| `CND-EVD-010` | causal, derivation, correction, or retraction target is invalid |
| `CND-EVD-011` | terminality is malformed or not final |

`conformance/c2/execution-event-v1.tsv` freezes causation, nested paths,
pressure/loss, redaction, correction, terminality, distributed timestamp
inversion, replay, malformed reference, semantic hash, and NDJSON round-trip
cases.

## Normative requirements

| ID | Obligation |
|---|---|
| EVD-001 | Link every event to one exact run and ExecutionPlan identity |
| EVD-002 | Keep append order, observer-local order, timestamps, and causality distinct |
| EVD-003 | Preserve immutable request, exchange, session, epoch, work, attempt, correlation, idempotency, checkpoint, and transport identities |
| EVD-004 | Preserve logical template and expanded replicated-instance paths |
| EVD-005 | Make cross-node causal and derivation relationships replayable |
| EVD-006 | Append corrections and retractions without mutating recorded events |
| EVD-007 | Apply bounded inline payload and structural redaction policy |
| EVD-008 | Never infer causality from wall or domain timestamps |
| EVD-009 | Keep encoding bytes, logs, projections, and presentation outside event identity |
| EVD-010 | Round-trip frozen NDJSON and in-memory records without semantic drift |
