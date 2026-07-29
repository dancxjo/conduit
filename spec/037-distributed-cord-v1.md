# Distributed cord protocol version 1

Status: normative C5 contract. `conformance/c5/distributed-cord-v1.json`
exercises every named scenario through the allocator-free reference state
machine; `conduit-runtime/tests/distributed_backend.rs` exercises the host
boundary and bounded fault backend.

## Identities and separation

An ordinary semantic cord remains an ordinary typed cord. Distribution is an
exact execution-plan binding selected because its endpoints resolved to
different hosts; it is not a new semantic port or value type (`DST-001`).

ExecutionPlan schema 9 carries one `PlanDistributedCord` for every cross-host
cord and none for a same-host cord (`DST-002`). The binding pins:

- the ordinary cord ID and both port-contract hashes;
- the complete resolved `FlowPolicy`;
- a session ID and initial session epoch;
- the selected backend and carrier-security descriptor;
- a carrier-owned binding distinct from the semantic cord ID;
- delivery, acknowledgement, ordering, reconnect, and disconnect policy;
- exact writer and reader host observations, realms, realm identities,
  entities, passports and schema versions, membership credentials, keys and
  key epochs, status/verifier providers, audiences, and grant hashes;
- an exact federation-policy pin for a cross-realm session; and
- all send, receive, retry, reorder, dedup, frame, heartbeat, reconnect,
  evidence, memory, timer, and transport allocations.

The binding has its own canonical identity. The enclosing plan hashes that
validated identity. The plan hash is deliberately not an input to the binding
hash, which avoids a self-referential identity.

## Handshake and security boundary

Values cannot flow until both peers agree on protocol version, sealed plan
identity, distributed-binding identity, cord, session, session epoch, run, and
run epoch (`DST-003`). The binding identity covers the port hashes, flow
policy, backend, carrier binding, delivery policy, peers, and budgets; a peer
cannot negotiate those fields independently after plan sealing.

Each side supplies fresh, identified observations proving:

1. the selected passport is currently active under the exact realm, entity,
   passport, status provider, time basis, and validity window;
2. the selected verification provider verified control of the expected
   membership credential using the session ID as the challenge; and
3. an unexpired workload delegation binds that passport, realm, entity, sealed
   plan, run, run epoch, and exact audience.

Rejected, replayed, conflicting-live-session, unavailable, expired, suspended,
revoked, retired, compromised, gap, wrong-audience, and wrong-plan proofs fail
closed. Reconnect revalidates live proofs; an immutable plan does not freeze a
formerly active status forever.

TLS, mTLS, QUIC, serial protection, WebTransport, WebRTC, WebSocket, Zenoh, or
another carrier may authenticate or protect bytes. Carrier authentication is
not realm membership, federation permission, workload delegation, or an
effect grant (`DST-004`). Plan validation independently requires exact
authority entries for the writer and reader endpoint nodes, selected host,
grant hash, and audience. Opening a session does not cache permission: open,
reconnect, send, receive, cancellation, and terminal operations each receive
fresh grant-status observations and re-run `check_at_use` for both sealed
endpoint grants. The effect and grant identities are recomputed rather than
trusted from caller-supplied hash fields. Revocation or a mutated grant
therefore fails before a carrier queue or evidence buffer is mutated.

## Delivery, ordering, and finite state

Version 1 offers only `at-most-once` and `at-least-once` (`DST-005`).
Exactly-once is intentionally absent because a transport acknowledgement is
not an application transaction commit.

At-most-once has no value acknowledgements, retransmission window, retry
attempts, or dedup window. At-least-once uses cumulative acknowledgements and
finite unacknowledged, retry, and optional dedup windows (`DST-006`). Losing an
acknowledgement may cause redelivery. A duplicate still within a declared
dedup window is suppressed; outside that window it is explicitly redelivered.
No result is described as exactly-once.

Sequence numbers are monotonic within one session epoch. In-order delivery may
hold only the declared finite reorder window. An out-of-window sequence,
oversized value, acknowledgement for unsent data, decreasing cumulative
acknowledgement, retry outside the retained window, or exhausted retry count
is rejected deterministically (`DST-007`).

The allocator-free `DistributedSessionMachine` retains counters and states but
never owns payload storage. Payload, retry, reorder, and carrier queues remain
caller- or backend-owned within the plan allocation.

## Disconnect and reconnect

A disconnect has one exact action: cancel the cord, fail the scope, or await a
bounded reconnect (`DST-008`). Awaiting reconnect is legal only with either:

- same-epoch resume, requiring the exact plan and binding identities, session
  epoch, writer/reader sequence positions, cumulative acknowledgement, and a
  nonzero proof receipt; or
- a new epoch exactly one greater than the old epoch, with sequence state
  reset and an explicit loss/duplication boundary.

Attempts and deadline ticks are finite. A reconnect never silently changes the
plan, host, endpoint, backend, delivery, sensitivity, authority, or security
contract. Moving an endpoint or backend belongs to a new plan epoch under
issue #57.

Heartbeat and liveness ticks share the exact named time basis selected for the
run. Liveness expiry produces an explicit partition observation and the
declared disconnect action (`DST-009`). Memory does not grow with partition
duration.

Cancellation and terminal frames remain pending until acknowledged or until a
separate finite policy resolves their loss (`DST-010`). Losing a terminal ack
does not turn a pending terminal state into proven closure. Cancellation and
terminal causes retain their existing semantic classes.

## Pressure and accounting

Every adapter-owned queue is finite and plan-visible (`DST-011`). The binding
separately budgets:

- send, receive, retry, reorder, and dedup item windows;
- send, receive, retry, and reorder bytes;
- maximum semantic payload and carrier frame bytes;
- maximum unacknowledged values, retry attempts, and reconnect attempts;
- heartbeat, liveness, and reconnect ticks;
- structured evidence records; and
- the exact memory, timer, transport, CPU, storage, checkpoint, and evidence
  allocation charged to the plan.

The semantic payload reservation is a lower bound on the carrier allocation.
Carrier framing, allocator, QoS, encryption, library, and kernel buffers must
fit the selected implementation profile and plan allocation too. Unknown or
unreconcilable carrier buffers reject the candidate during resolution.

Transport buffering does not replace the ordinary cord queue. A backend maps
readiness and saturation to the exact cord `FlowPolicy`; it cannot silently
block a rejecting cord, discard a lossless value, or add an unbounded retry
log. Version 1 does not introduce a new pressure behavior.

## Backend boundary and evidence

`DistributedCordBackend` accepts the already resolved binding, live handshake,
and validation context; requires fresh authority context at every effectful
operation; exposes send/receive readiness; uses caller-provided receive
storage; propagates cancellation and terminal closure; and yields structured
evidence (`DST-012`). The interface requires no Tokio, `async_trait`, carrier
library type, operating system, or caller allocation.

The bounded in-memory backend is the deterministic fault reference. It can
drop an acknowledgement, duplicate a value, reorder one value pair, and expose
a partition while enforcing receive item/byte, payload/frame, and evidence
ceilings. It is not represented as a production network carrier. Zenoh and
carrier-level conformance remain owned by issue #41.

Transport observations include the plan and binding identities, cord,
session/epoch, value sequence, retry attempt, correlation identity, kind, and
stable reason (`DST-013`). Local and remote recorders can therefore correlate
one session without claiming a universal total order. These observations are
not semantic values, durable Resonance events, or `ExecutionEvent` records
until an explicit bounded evidence projection records them.

Distributed Resonance streams remain distinct (`DST-014`). A carrier may
implement both a live distributed cord and a retained event stream, but a cord
does not gain retention, replay, cursor, gap, or subscriber semantics
implicitly.

Oversized frames are rejected before backend-owned payload allocation.
Capacity arithmetic is checked, evidence is finite, and failure retains a
stable `CND-DST-*` reason (`DST-015`).

## Stable reasons

- `CND-DST-001` unsupported protocol or binding version
- `CND-DST-002` binding identity mismatch
- `CND-DST-003` malformed plan binding
- `CND-DST-004` binding does not match the exact cord flow or ports
- `CND-DST-005` unbounded, inconsistent, or under-accounted budget
- `CND-DST-006` unsupported delivery/acknowledgement combination
- `CND-DST-007` peer, realm, federation, or host-observation mismatch
- `CND-DST-008` stale or non-active passport status
- `CND-DST-009` rejected possession/credential proof
- `CND-DST-010` invalid workload delegation
- `CND-DST-011` missing or mismatched authority
- `CND-DST-012` handshake identity mismatch
- `CND-DST-013` oversized payload or frame
- `CND-DST-014` sequence or acknowledgement violation
- `CND-DST-015` exhausted retry window or attempt count
- `CND-DST-016` duplicate outside a policy that permits redelivery
- `CND-DST-017` reconnect forbidden or attempt count exhausted
- `CND-DST-018` resume/new-epoch proof mismatch
- `CND-DST-019` illegal terminal/cancellation transition
- `CND-DST-020` finite transport buffer full
- `CND-DST-021` explicit partition
- `CND-DST-022` finite evidence buffer full

## Migration

ExecutionPlan schemas 1 through 8 remain readable with their frozen identities
and cannot contain distributed bindings (`DST-016`). They also cannot run a
cross-host cord under schema-9 rules. A planner migrating a genuinely
cross-host plan must create schema 9, supply all exact peer/authority/backend
facts and budgets, and recompute plan identity. No older local cord is
reinterpreted as distributed from ambient host state.

Schema 9 and distributed-binding schema 1 remain frozen. Specification 040
defines plan schema 10 and distributed-binding schema 2 for the additional
implementation artifact, execution profile, explicit carrier protection, and
carrier endpoint pins. A schema-9 binding is never upgraded in place or
interpreted as schema 2.
