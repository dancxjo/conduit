# Zenoh reference transport v1

Status: candidate C5 contract
Issue: #41
Depends on: specifications 010, 011, 012, 017, 022, 023, 025, 027, 031,
032, 033, 037, and 038

## Boundary

Zenoh is one host implementation of the distributed-cord contract. It is not
a semantic node, port, cord, realm, grant, event stream, network attachment,
or lifecycle authority. No Zenoh type or configuration value enters
`conduit-core`.

The identities remain distinct:

1. the semantic cord and port contracts;
2. the plan-v10 distributed binding;
3. the selected implementation and artifact;
4. the execution profile and fresh host report;
5. the carrier endpoint and key expression;
6. carrier authentication/encryption;
7. Conduit realm/passport proofs and authority grants; and
8. run evidence.

The adapter receives an already resolved network endpoint. It MUST NOT scan
for Wi-Fi, associate a station, host an AP, assign an address, install a route,
change a firewall, issue a certificate, install a router, or provision a host.
Those are separate authored services.

## Exact plan revision

Distributed-binding schema 1 remains the frozen plan-v9 representation.
Plan-v10 requires distributed-binding schema 2. Schema 2 adds:

- exact backend artifact ID and digest;
- exact execution-profile descriptor; and
- explicit carrier protection mode; and
- exact carrier endpoint.

The backend implementation descriptor, carrier-security descriptor, carrier
binding/key expression, delivery, ordering, acknowledgement, reconnect,
disconnect, queue/retry/dedup limits, endpoint peer requirements, grants,
host observations, and allocation remain pinned. A schema-2 artifact MUST be
present exactly in the plan artifact set. Missing or mixed revisions reject;
schema 1 is not reinterpreted.

## Runtime interface

`DistributedCordBackend` is carrier-neutral and executor-neutral. It exposes
capabilities, open and reauthentication, nonblocking readiness, send and
caller-buffer receive, cancellation, terminal close, and finite structured
evidence. It requires neither Tokio nor `async_trait`, allocation by the
caller, nor any Zenoh type.

The wire envelope is bounded by the selected frame ceiling and carries:

- protocol version;
- exact plan and distributed-binding identities;
- cord and session IDs;
- session epoch;
- frame kind, sequence, attempt, and correlation; and
- a length-delimited payload.

Malformed, oversized, wrong-plan, wrong-binding, wrong-cord, wrong-session,
and stale-epoch envelopes fail before payload delivery.

## Hosted Zenoh profile

The hosted reference uses Zenoh 1.9.0 with explicit peer mode and exact
listen/connect endpoint. Multicast and gossip scouting are disabled. The
adapter declares a remote-only publisher/subscriber pair for the pinned key
expression. The callback uses `try_send` into a finite synchronous channel and
never blocks a Zenoh callback thread.

This profile implements publish/subscribe only. A host capability report that
claims query/reply for this implementation is rejected before network I/O.

The selected profile accounts separately for adapter send/receive/evidence,
Zenoh priority queues and frame storage, receive and defragmentation buffers,
socket send/receive buffers, session/discovery state, pending operations,
retained payload, timers, links, sessions, and retry timers. Configurable
Zenoh and socket ceilings are applied to the session. Any dependency/library
memory that is only observed makes the complete profile `Observed`, not
`Hard`; the current hosted reference therefore does not claim a universal
heap hard limit.

Plaintext, server-authenticated TLS, and mutually authenticated TLS are
distinct selected modes. TLS secret paths are opaque host handles and are not
included in plan identity, diagnostics, or evidence. Missing material rejects
before network I/O. A transition may retain or strengthen carrier security
but MUST NOT weaken it. Every changed exact binding requires a greater session
epoch.

Carrier evidence states the selected protection, whether the carrier peer was
authenticated, whether authentication was mutual, and whether bytes were
encrypted. A separate field records that fresh Conduit authority validation
occurred. Neither field implies the other.

For one-way TLS, the connector authenticates the listening peer; the listener
MUST NOT claim that the unauthenticated client peer was authenticated. mTLS
records peer authentication on both ends.

## Deterministic oracle and failures

The bounded in-memory backend is the semantic oracle. It independently
executes duplicate delivery, reordering, acknowledgement loss, terminal
acknowledgement loss, partition, reconnect, cancellation, queue exhaustion,
and oversized-frame cases. A real hosted Zenoh exchange MUST normalize to the
same kind, epoch, sequence, attempt, correlation, and payload outcome.

Zenoh reconnection does not manufacture Conduit progress. Resume or new-epoch
admission revalidates the exact handshake, live passport status, possession,
workload delegation, and both endpoint grants. Reconnect attempts and
evidence are finite.

## Firmware and Zenoh-Pico path

The same semantic transport requirement may resolve to:

- `conduit/transport.zenoh-rust` with `NativeInProcess`; or
- `conduit/transport.zenoh-pico` with `Firmware`.

The firmware manifest uses the general
`conduit/embedded-host-service-v1` adapter and
`conduit/ffi-message-v1` ABI. Fixed request/reply values cross the existing
allocator-free `EmbeddedHostServices` boundary. There is no privileged
Zenoh-specific C loader, Python path, or core API.

A fresh RP2040 report must explicitly advertise the firmware executor, target,
ABI, artifact, static memory, queues, timers, link/topology facts, security
mode, and Zenoh-Pico transport capability. If any fact is absent, resolution
rejects deterministically. This repository proves the manifest/resolver and
general firmware-boundary path; it does not claim that every Pico firmware
contains Zenoh-Pico or that physical radio interoperability was exercised.

## Resonance and network composition

The hosted profile in this specification is a live distributed-cord provider,
not a durable Resonance provider. Event-stream-only and combined plans require
a separate exact event-provider binding. Zenoh storage/query/history may be
selected only when a provider manifest proves the requested retention,
replay, cursor, integrity, redaction, security, and finite-resource contract.
The live profile deterministically rejects those claims.

Network attachment readiness, address readiness, and Zenoh session readiness
are separate observations. Link loss or address replacement may require
drain/discontinuity and a new transport session epoch. Transport cancellation
and shutdown do not reconfigure the network attachment.

## Requirements

| ID | Requirement |
| --- | --- |
| ZEN-001 | Keep Zenoh and network-attachment concepts out of `conduit-core`. |
| ZEN-002 | Plan-v10 pins the complete schema-2 transport selection without reinterpreting plan-v9. |
| ZEN-003 | Expose a carrier-neutral readiness/send/receive/cancel/close interface. |
| ZEN-004 | Bound and account every adapter-owned queue, retry window, buffer, timer, link, and evidence store. |
| ZEN-005 | Classify dependency and kernel bounds honestly; observed-only profiles are not hard bounds. |
| ZEN-006 | Preserve plan, binding, cord, session, epoch, sequence, attempt, and correlation in every envelope. |
| ZEN-007 | Normalize a real hosted Zenoh exchange against the deterministic oracle. |
| ZEN-008 | Execute duplicate, reorder, lost-ack, partition, reconnect, cancellation, terminal-ack-loss, and oversize failures independently. |
| ZEN-009 | Make plaintext, TLS, and mTLS explicit and prohibit silent downgrade. |
| ZEN-010 | Distinguish carrier protection from Conduit authority evidence. |
| ZEN-011 | Resolve exact implementation, artifact, profile, endpoint, host, and capability facts. |
| ZEN-012 | Use the general firmware host-service/message ABI for the Zenoh-Pico path and reject unsupported hosts. |
| ZEN-013 | Reauthenticate realm/passport/delegation/grant inputs on admission and reconnect. |
| ZEN-014 | Keep live cords, Resonance providers, and network attachment as separate exact bindings. |
| ZEN-015 | Require explicit discontinuity/new epoch for changed transport bindings and forbid weakened guarantees. |

## Stable transport reasons

`CND-TRN-001` through `CND-TRN-016` cover unsupported protocol, binding,
implementation, artifact, profile, endpoint, security, accounting, envelope,
queue, carrier, disconnection, secret-handle, and firmware-ABI failures.
Distributed semantic failures retain the `CND-DST-*` vocabulary.

## Proof boundary

Conformance includes independently dispatched plan/binding, selection,
envelope, fault, security, resolver, firmware-path, realm/passport,
replacement, Resonance-separation, and network-separation cases. The hosted
reference tests open real loopback plaintext, TLS, and mTLS Zenoh sessions.
Loopback is integration proof of the carrier adapter, not evidence of a
physical Pico radio, router deployment, Internet reachability, or network
provisioning.
